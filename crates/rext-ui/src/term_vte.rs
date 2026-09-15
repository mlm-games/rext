//! `vte::Perform` implementation: feeds ANSI byte streams into a `TermGrid`.
//! Covers the sequences interactive shells actually emit: SGR colors/attrs,
//! cursor motion, erase, scroll regions (margined scroll = status bars), alt
//! screen, bracketed paste markers (ignored), OSC title (stored), charset
//! selection (ignored — UTF-8 assumed).

use crate::term_grid::{CellAttr, SharedGrid};
use std::sync::{Arc, Mutex};

pub struct GridPerform {
    pub grid: SharedGrid,
    pub title: Arc<Mutex<String>>,
    /// Pending scroll region (top, bottom) 0-based inclusive. None = full.
    region: Option<(usize, usize)>,
    /// Application cursor mode (solitary effect: none on grid, tracked).
    pub app_cursor: bool,
    /// Bracketed paste (we just don't wrap pastes).
    pub bracketed_paste: bool,
}

impl GridPerform {
    pub fn new(grid: SharedGrid) -> Self {
        Self {
            grid,
            title: Arc::new(Mutex::new(String::new())),
            region: None,
            app_cursor: false,
            bracketed_paste: false,
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) {
        let mut parser = vte::Parser::new();
        parser.advance(self, bytes);
    }
}

fn sgr_color(attr: &mut CellAttr, code: u16, bright: bool, is_fg: bool) {
    let idx = if bright { code + 8 } else { code } as u8;
    if is_fg {
        attr.fg_kind = 1;
        attr.fg = idx as u32;
    } else {
        attr.bg_kind = 1;
        attr.bg = idx as u32;
    }
}

impl vte::Perform for GridPerform {
    fn print(&mut self, c: char) {
        if let Ok(mut g) = self.grid.lock() {
            g.put(c);
        }
    }

    fn execute(&mut self, byte: u8) {
        let Ok(mut g) = self.grid.lock() else {
            return;
        };
        match byte {
            b'\n' | 0x0B | 0x0C => g.put('\n'),
            b'\r' => g.put('\r'),
            0x08 => g.backspace(),
            b'\t' => g.put('\t'),
            0x07 => {} // BEL: ignore (no audio yet)
            _ => {}
        }
    }

    fn hook(
        &mut self,
        _params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
    }
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        // OSC 0/2: window title.
        if params.len() >= 2 {
            let code = std::str::from_utf8(params[0]).unwrap_or("");
            if code == "0" || code == "2" {
                if let Ok(t) = std::str::from_utf8(params[1]) {
                    if let Ok(mut title) = self.title.lock() {
                        *title = t.to_string();
                    }
                }
            }
        }
    }
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        let p: Vec<u16> = params.iter().map(|s| s.first().copied().unwrap_or(0)).collect();
        let get = |i: usize, dflt: u16| p.get(i).copied().unwrap_or(dflt);
        let Ok(mut g) = self.grid.lock() else {
            return;
        };
        match action {
            'A' => g.move_by(-(get(0, 1) as isize), 0),
            'B' => g.move_by(get(0, 1) as isize, 0),
            'C' => g.move_by(0, get(0, 1) as isize),
            'D' => g.move_by(0, -(get(0, 1) as isize)),
            'E' => {
                let n = get(0, 1) as isize;
                g.move_by(n, 0);
                let r = g.cursor.0;
                g.move_to(r, 0);
            }
            'F' => {
                let n = get(0, 1) as isize;
                g.move_by(-n, 0);
                let r = g.cursor.0;
                g.move_to(r, 0);
            }
            'G' | '`' => {
                let c = get(0, 1).saturating_sub(1) as usize;
                let r = g.cursor.0;
                g.move_to(r, c);
            }
            'H' | 'f' => {
                let r = get(0, 1).saturating_sub(1) as usize;
                let c = get(1, 1).saturating_sub(1) as usize;
                g.move_to(r, c);
            }
            'J' => match get(0, 0) {
                0 => {
                    // cursor → end
                    let (r, c) = g.cursor;
                    g.clear_line(0);
                    // crude: clear rows below
                    let rows = g.rows;
                    for rr in (r + 1)..rows {
                        g.move_to(rr, 0);
                        g.clear_line(2);
                    }
                    g.move_to(r, c);
                }
                1 => {
                    let (r, c) = g.cursor;
                    g.clear_line(1);
                    for rr in 0..r {
                        g.move_to(rr, 0);
                        g.clear_line(2);
                    }
                    g.move_to(r, c);
                }
                _ => g.clear_screen(),
            },
            'K' => g.clear_line(get(0, 0) as u8),
            'L' => {
                // insert lines: push blank rows at cursor within region
                let n = get(0, 1) as usize;
                let (r, c) = g.cursor;
                for _ in 0..n {
                    // simple: clear current line and shift down via newlines
                    g.move_to(r, 0);
                    g.clear_line(2);
                }
                g.move_to(r, c);
            }
            'M' => {
                // delete lines
                let (r, c) = g.cursor;
                for _ in 0..get(0, 1) {
                    g.move_to(r, 0);
                    g.clear_line(2);
                }
                g.move_to(r, c);
            }
            'P' => {
                // delete chars
                let (r, c) = g.cursor;
                g.move_to(r, c);
                g.clear_line(0);
                g.move_to(r, c);
            }
            'S' => {
                for _ in 0..get(0, 1) {
                    g.put('\n');
                }
            }
            'T' => {
                g.move_by(-(get(0, 1) as isize), 0);
            }
            'd' => {
                let r = get(0, 1).saturating_sub(1) as usize;
                let c = g.cursor.1;
                g.move_to(r, c);
            }
            'm' => {
                let mut attr = g.cur_attr();
                if p.is_empty() {
                    attr = CellAttr::plain();
                }
                let mut i = 0;
                while i < p.len() {
                    let code = p[i];
                    match code {
                        0 => attr = CellAttr::plain(),
                        1 => attr.bold = true,
                        4 => attr.underline = true,
                        22 => attr.bold = false,
                        24 => attr.underline = false,
                        30..=37 => sgr_color(&mut attr, code - 30, false, true),
                        39 => {
                            attr.fg_kind = 0;
                            attr.fg = 0;
                        }
                        40..=47 => sgr_color(&mut attr, code - 40, false, false),
                        49 => {
                            attr.bg_kind = 0;
                            attr.bg = 0;
                        }
                        90..=97 => sgr_color(&mut attr, code - 90, true, true),
                        100..=107 => sgr_color(&mut attr, code - 100, true, false),
                        _ => {}
                    }
                    i += 1;
                }
                g.set_attr(attr);
            }
            'r' => {
                // scroll region
                let top = get(0, 1).saturating_sub(1) as usize;
                let bot = get(1, 1000).saturating_sub(1) as usize;
                self.region = Some((top, bot));
                g.move_to(0, 0);
            }
            's' => {
                // save cursor (DEC) — grid has no API; ignore (alt-screen path covers apps)
            }
            'u' => {}
            'h' | 'l' => {
                // private modes: ?1049 alt screen, ?2004 bracketed paste, ?1 app cursor
                let on = action == 'h';
                for mode in p.iter() {
                    match mode {
                        1049 | 1047 | 1048 => {
                            if on {
                                g.enter_alt();
                            } else {
                                g.exit_alt();
                            }
                        }
                        2004 => self.bracketed_paste = on,
                        1 => self.app_cursor = on,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
