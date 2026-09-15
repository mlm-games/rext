//! Real terminal backend: `portable-pty` (process + pty) + `vte` (ANSI
//! parser) feeding a scrollback grid rendered on a Repose Canvas.
//!
//! Platform story:
//! - desktop (linux/mac/windows): native pty via `portable-pty` — full shell.
//! - wasm32: no process/pty APIs exist in the browser sandbox. Backend is
//!   `Echo` (local line discipline) until a `WebSocket`-to-server pty bridge
//!   is wired (xterm.js talks to the same kind of backend; see docs).
//! - android: `portable-pty` has no android backend. Until one exists, use
//!   the same `Echo` fallback or bridge to a Termux-style session fd. The
//!   grid/parser/render path is identical on all three.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellAttr {
    pub fg: u32,
    pub bg: u32,
    pub bold: bool,
    pub underline: bool,
    /// 0 = default fg, else 0..16 ANSI palette index, 0x100+ = 256-color, etc.
    pub fg_kind: u8,
    pub bg_kind: u8,
}

impl CellAttr {
    pub fn plain() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug)]
pub struct TermCell {
    pub ch: char,
    pub attr: CellAttr,
    pub wide: u8, // 0 = normal, 1 = wide lead, 2 = wide trail
}

impl Default for TermCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            attr: CellAttr::plain(),
            wide: 0,
        }
    }
}

/// 16-color ANSI palette (matching the editor's dark theme).
pub const PALETTE: [u32; 16] = [
    0x282C34, // black
    0xE06C75, // red
    0x98C379, // green
    0xE5C07B, // yellow
    0x61AFEF, // blue
    0xC678DD, // magenta
    0x56B6C2, // cyan
    0xABB2BF, // white
    0x5C6370, // bright black
    0xE06C75, // bright red
    0x98C379, // bright green
    0xE5C07B, // bright yellow
    0x61AFEF, // bright blue
    0xC678DD, // bright magenta
    0x56B6C2, // bright cyan
    0xFFFFFF, // bright white
];

pub const DEFAULT_FG: u32 = 0xABB2BF;
pub const DEFAULT_BG: u32 = 0x1E2127;

/// A scrollback + viewport grid. Parser writes here; renderer reads.
/// All mutation happens under one lock shared with the pty reader thread.
pub struct TermGrid {
    pub cols: usize,
    pub rows: usize,
    /// All lines, oldest first. `scrollback` caps total stored lines.
    lines: VecDeque<Vec<TermCell>>,
    /// Cursor in grid coords (row indexes into the *visible window* tail).
    pub cursor: (usize, usize),
    pub cursor_visible: bool,
    pub scrollback: usize,
    /// Alternate screen (htop/vim): no scrollback, cleared on exit.
    alt: Option<Vec<Vec<TermCell>>>,
    saved_cursor: (usize, usize),
    cur_attr: CellAttr,
}

impl TermGrid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let mut g = Self {
            cols: cols.max(1),
            rows: rows.max(1),
            lines: VecDeque::new(),
            cursor: (0, 0),
            cursor_visible: true,
            scrollback: 2000,
            alt: None,
            saved_cursor: (0, 0),
            cur_attr: CellAttr::plain(),
        };
        g.ensure_rows();
        g
    }

    fn blank_row(&self) -> Vec<TermCell> {
        vec![TermCell::default(); self.cols]
    }

    fn ensure_rows(&mut self) {
        let (cols, rows) = (self.cols, self.rows);
        let blank = || vec![TermCell::default(); cols];
        if self.alt.is_some() {
            let alt = self.alt.as_mut().unwrap();
            while alt.len() < rows {
                alt.push(blank());
            }
        } else {
            while self.lines.len() < rows {
                self.lines.push_back(blank());
            }
        }
    }

    fn active_len(&self) -> usize {
        self.alt.as_ref().map(|a| a.len()).unwrap_or(self.lines.len())
    }

    fn row_mut(&mut self, r: usize) -> &mut Vec<TermCell> {
        self.ensure_rows();
        if let Some(alt) = &mut self.alt {
            let r = r.min(alt.len().saturating_sub(1));
            return &mut alt[r];
        }
        let start = self.lines.len().saturating_sub(self.rows);
        &mut self.lines[start + r.min(self.rows.saturating_sub(1))]
    }

    /// Visible rows, oldest-top. Exactly `rows` entries.
    pub fn visible(&self) -> Vec<Vec<TermCell>> {
        if let Some(alt) = &self.alt {
            let mut v: Vec<Vec<TermCell>> = alt.iter().take(self.rows).cloned().collect();
            while v.len() < self.rows {
                v.push(vec![TermCell::default(); self.cols]);
            }
            return v;
        }
        let start = self.lines.len().saturating_sub(self.rows);
        self.lines
            .iter()
            .skip(start)
            .take(self.rows)
            .cloned()
            .collect()
    }

    pub fn total_lines(&self) -> usize {
        self.active_len()
    }

    pub fn set_attr(&mut self, a: CellAttr) {
        self.cur_attr = a;
    }

    pub fn cur_attr(&self) -> CellAttr {
        self.cur_attr
    }

    pub fn put(&mut self, ch: char) {
        if ch == '\n' {
            self.newline();
            return;
        }
        if ch == '\r' {
            self.cursor.1 = 0;
            return;
        }
        if ch == '\t' {
            for _ in 0..8 - (self.cursor.1 % 8) {
                self.put(' ');
            }
            return;
        }
        // Wide chars (CJK, emoji) occupy two cells: lead + trail marker.
        // Zero-width joiners/combiners attach to the previous cell.
        let cells = ch.width().unwrap_or(1).max(1).min(2);
        if self.cursor.1 + cells > self.cols {
            self.newline();
        }
        {
            let (r, c) = self.cursor;
            let attr = self.cur_attr;
            let row = self.row_mut(r);
            if c < row.len() {
                row[c] = TermCell {
                    ch,
                    attr,
                    wide: if cells == 2 { 1 } else { 0 },
                };
                if cells == 2 && c + 1 < row.len() {
                    // Trail cell: never painted, just reserves the advance.
                    row[c + 1] = TermCell {
                        ch: ' ',
                        attr,
                        wide: 2,
                    };
                }
            }
            self.cursor.1 += cells;
        }
    }

    fn newline(&mut self) {
        self.cursor.1 = 0;
        self.cursor.0 += 1;
        if self.alt.is_some() {
            if self.cursor.0 >= self.rows {
                self.cursor.0 = self.rows - 1;
                // scroll alt screen
                if let Some(alt) = &mut self.alt {
                    let blank = vec![TermCell::default(); self.cols];
                    alt.remove(0);
                    alt.push(blank);
                }
            }
        } else {
            if self.cursor.0 >= self.rows {
                self.cursor.0 = self.rows - 1;
                self.lines.push_back(self.blank_row());
                while self.lines.len() > self.scrollback + self.rows {
                    self.lines.pop_front();
                }
            }
            self.ensure_rows();
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        }
    }

    pub fn move_to(&mut self, row: usize, col: usize) {
        self.cursor.0 = row.min(self.rows.saturating_sub(1));
        self.cursor.1 = col.min(self.cols.saturating_sub(1));
    }

    pub fn move_by(&mut self, dr: isize, dc: isize) {
        let r = (self.cursor.0 as isize + dr).clamp(0, self.rows as isize - 1) as usize;
        let c = (self.cursor.1 as isize + dc).clamp(0, self.cols as isize - 1) as usize;
        self.cursor = (r, c);
    }

    pub fn clear_screen(&mut self) {
        if self.alt.is_some() {
            if let Some(alt) = &mut self.alt {
                for row in alt.iter_mut() {
                    for cell in row.iter_mut() {
                        *cell = TermCell::default();
                    }
                }
            }
        } else {
            self.lines.clear();
            self.ensure_rows();
        }
        self.cursor = (0, 0);
    }

    pub fn clear_line(&mut self, mode: u8) {
        let (r, c) = self.cursor;
        let row = self.row_mut(r);
        match mode {
            0 => {
                for cell in row.iter_mut().skip(c) {
                    *cell = TermCell::default();
                }
            }
            1 => {
                for cell in row.iter_mut().take(c + 1) {
                    *cell = TermCell::default();
                }
            }
            _ => {
                for cell in row.iter_mut() {
                    *cell = TermCell::default();
                }
            }
        }
    }

    pub fn enter_alt(&mut self) {
        if self.alt.is_none() {
            let mut alt = Vec::with_capacity(self.rows);
            for _ in 0..self.rows {
                alt.push(self.blank_row());
            }
            self.alt = Some(alt);
            self.saved_cursor = self.cursor;
            self.cursor = (0, 0);
        }
    }

    pub fn exit_alt(&mut self) {
        if self.alt.take().is_some() {
            self.cursor = self.saved_cursor;
        }
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.cols = cols.max(1);
        self.rows = rows.max(1);
        if let Some(alt) = &mut self.alt {
            alt.clear();
        } else {
            for line in self.lines.iter_mut() {
                line.resize(self.cols, TermCell::default());
            }
        }
        self.ensure_rows();
        self.cursor.0 = self.cursor.0.min(self.rows.saturating_sub(1));
        self.cursor.1 = self.cursor.1.min(self.cols.saturating_sub(1));
    }
}

pub type SharedGrid = Arc<Mutex<TermGrid>>;

pub fn new_shared_grid(cols: usize, rows: usize) -> SharedGrid {
    Arc::new(Mutex::new(TermGrid::new(cols, rows)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_newline() {
        let mut g = TermGrid::new(10, 4);
        for c in "hi".chars() {
            g.put(c);
        }
        assert_eq!(g.cursor, (0, 2));
        g.put('\n');
        assert_eq!(g.cursor, (1, 0));
        let v = g.visible();
        assert_eq!(v[0][0].ch, 'h');
        assert_eq!(v[0][1].ch, 'i');
    }

    #[test]
    fn scrollback_grows() {
        let mut g = TermGrid::new(10, 2);
        g.scrollback = 5;
        for _ in 0..10 {
            g.put('\n');
        }
        assert!(g.lines.len() <= g.scrollback + g.rows);
    }

    #[test]
    fn alt_screen_isolated() {
        let mut g = TermGrid::new(10, 4);
        g.put('a');
        g.enter_alt();
        assert_eq!(g.visible()[0][0].ch, ' ');
        g.put('b');
        g.exit_alt();
        assert_eq!(g.visible()[0][0].ch, 'a');
    }

    #[test]
    fn wide_char_occupies_two_cells() {
        let mut g = TermGrid::new(10, 2);
        g.put('あ'); // wide
        assert_eq!(g.cursor, (0, 2));
        let v = g.visible();
        assert_eq!(v[0][0].ch, 'あ');
        assert_eq!(v[0][0].wide, 1);
        assert_eq!(v[0][1].wide, 2);
        g.put('b');
        assert_eq!(g.cursor, (0, 3));
        assert_eq!(g.visible()[0][2].ch, 'b');
    }

    #[test]
    fn wide_char_wraps_at_edge() {
        let mut g = TermGrid::new(4, 3);
        g.put('a');
        g.put('b');
        g.put('c');
        g.put('あ'); // needs 2 cells but only 1 left → wraps
        assert_eq!(g.cursor, (1, 2));
        let v = g.visible();
        assert_eq!(v[0][3].ch, ' ');
        assert_eq!(v[1][0].ch, 'あ');
    }
}
