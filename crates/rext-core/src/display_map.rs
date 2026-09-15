//! Buffer → display rows.
//!
//! Soft-wrap maps one buffer row to N display rows when the expanded line
//! exceeds the wrap width. Column units here are **display columns**: expanded
//! chars after tab expansion (tab = advance to next multiple of tab_width).
//! The UI layer converts display columns to px via the metrics cache, so this
//! module stays font-independent and works with proportional fallback glyphs.
//!
//! This is the seam tree-sitter / folds / inlays plug into later.

use crate::buffer::{Buffer, Point};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisplayPoint {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum SoftWrap {
    Off,
    /// Wrap at this pixel width. The UI converts px → display columns via
    /// measured advance and calls [`DisplayMap::set_wrap_cols`].
    EditorWidth(f32),
    /// Wrap at a fixed display-column count (testing / fallback).
    Column(usize),
}

pub struct DisplayMap {
    pub soft_wrap: SoftWrap,
    pub tab_width: usize,
    /// Display columns per wrap row, derived from `SoftWrap::EditorWidth`
    /// by the UI layer. `None` = no wrapping.
    wrap_cols: Option<usize>,
}

impl Default for DisplayMap {
    fn default() -> Self {
        Self {
            soft_wrap: SoftWrap::Off,
            tab_width: 4,
            wrap_cols: None,
        }
    }
}

impl DisplayMap {
    /// Set the wrap width in display columns (UI calls this after measuring).
    /// Pass `None` to disable wrapping regardless of `soft_wrap`.
    pub fn set_wrap_cols(&mut self, cols: Option<usize>) {
        self.wrap_cols = cols.filter(|&c| c > 0);
    }

    fn active_wrap_cols(&self) -> Option<usize> {
        match self.soft_wrap {
            SoftWrap::Off => None,
            SoftWrap::Column(c) if c > 0 => Some(c),
            SoftWrap::Column(_) => None,
            SoftWrap::EditorWidth(_) => self.wrap_cols,
        }
    }

    /// Visual width of a char at display column `col` (tabs advance to the
    /// next tab stop, everything else is 1 display column).
    fn char_cells(&self, ch: char, col: usize) -> usize {
        if ch == '\t' {
            self.tab_width - (col % self.tab_width)
        } else {
            1
        }
    }

    /// Expanded line for paint (tabs → spaces). No wrap yet.
    pub fn display_line(&self, buf: &Buffer, row: usize) -> String {
        let raw = buf.line(row);
        if self.tab_width <= 1 || !raw.contains('\t') {
            return raw;
        }
        let mut out = String::with_capacity(raw.len());
        let mut col = 0usize;
        for ch in raw.chars() {
            if ch == '\t' {
                let spaces = self.tab_width - (col % self.tab_width);
                for _ in 0..spaces {
                    out.push(' ');
                }
                col += spaces;
            } else {
                out.push(ch);
                col += 1;
            }
        }
        out
    }

    /// Display-column width of a buffer row after tab expansion.
    pub fn row_width_cols(&self, buf: &Buffer, row: usize) -> usize {
        let line = buf.line(row);
        let mut col = 0usize;
        for ch in line.chars() {
            col += self.char_cells(ch, col);
        }
        col
    }

    /// Number of display rows a buffer row occupies (≥ 1).
    pub fn wrap_count(&self, buf: &Buffer, row: usize) -> usize {
        match self.active_wrap_cols() {
            None => 1,
            Some(w) => {
                let width = self.row_width_cols(buf, row);
                if width == 0 {
                    1
                } else {
                    width.div_ceil(w)
                }
            }
        }
    }

    /// Buffer row → first display row. O(row); the surface calls this once
    /// per frame for the scroll offset and iterates forward from there.
    pub fn display_row_of(&self, buf: &Buffer, row: usize) -> usize {
        let row = row.min(buf.len_lines().saturating_sub(1));
        let mut d = 0usize;
        for r in 0..row {
            d += self.wrap_count(buf, r);
        }
        d
    }

    /// Total display rows in the buffer.
    pub fn display_row_count(&self, buf: &Buffer) -> usize {
        self.display_row_of(buf, buf.len_lines().saturating_sub(1))
            + self.wrap_count(buf, buf.len_lines().saturating_sub(1))
    }

    /// Display row → (buffer row, wrap index within that row).
    pub fn buffer_row_of(&self, buf: &Buffer, drow: usize) -> (usize, usize) {
        let mut d = 0usize;
        for r in 0..buf.len_lines().max(1) {
            let n = self.wrap_count(buf, r);
            if drow < d + n {
                return (r, drow - d);
            }
            d += n;
        }
        let last = buf.len_lines().saturating_sub(1);
        (last, self.wrap_count(buf, last).saturating_sub(1))
    }

    /// Byte-column ranges (display columns) of each wrap segment of a row.
    /// Used by paint to slice the expanded line per display row.
    pub fn wrap_segments(&self, buf: &Buffer, row: usize) -> Vec<(usize, usize)> {
        let width = self.row_width_cols(buf, row);
        match self.active_wrap_cols() {
            None => vec![(0, width)],
            Some(w) => {
                let mut segs = Vec::new();
                let mut start = 0usize;
                while start < width.max(1) {
                    let end = (start + w).min(width);
                    segs.push((start, end));
                    start = end;
                    if width == 0 {
                        break;
                    }
                }
                if segs.is_empty() {
                    segs.push((0, 0));
                }
                segs
            }
        }
    }

    /// Slice of the expanded line for wrap segment `wrap_idx` of `row`.
    pub fn display_segment(&self, buf: &Buffer, row: usize, wrap_idx: usize) -> String {
        let expanded = self.display_line(buf, row);
        let segs = self.wrap_segments(buf, row);
        let Some(&(s, e)) = segs.get(wrap_idx) else {
            return String::new();
        };
        // Expanded line is spaces for tabs, so display cols == char cols.
        let chars: Vec<char> = expanded.chars().collect();
        chars
            .get(s..e.min(chars.len()))
            .unwrap_or(&[])
            .iter()
            .collect()
    }

    /// Buffer point → (display row, display column within that row).
    /// Unchanged behavior when wrap is off (1:1 rows).
    pub fn point_to_display(&self, buf: &Buffer, p: Point) -> DisplayPoint {
        let row = p.row.min(buf.len_lines().saturating_sub(1));
        let line = buf.line(row);
        // display column of the buffer column (tabs expanded)
        let mut col = 0usize;
        for (i, ch) in line.chars().enumerate() {
            if i >= p.column {
                break;
            }
            col += self.char_cells(ch, col);
        }
        match self.active_wrap_cols() {
            None => DisplayPoint { row, column: col },
            Some(w) => {
                let first = self.display_row_of(buf, row);
                DisplayPoint {
                    row: first + col / w,
                    column: col % w,
                }
            }
        }
    }

    /// (Display row, column within row) → buffer point. Inverse of
    /// [`point_to_display`](Self::point_to_display).
    pub fn display_to_point(&self, buf: &Buffer, dp: DisplayPoint) -> Point {
        let (row, col) = match self.active_wrap_cols() {
            None => (dp.row.min(buf.len_lines().saturating_sub(1)), dp.column),
            Some(w) => {
                let (r, wrap_idx) = self.buffer_row_of(buf, dp.row);
                (r, wrap_idx * w + dp.column)
            }
        };
        let line = buf.line(row);
        let mut dcol = 0usize;
        let mut col_idx = 0usize;
        for ch in line.chars() {
            let wch = self.char_cells(ch, dcol);
            if dcol + wch > col {
                break;
            }
            dcol += wch;
            col_idx += 1;
        }
        Point {
            row,
            column: col_idx.min(buf.line_len(row)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_with(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn off_is_identity() {
        let b = buf_with("hello\nworld");
        let m = DisplayMap::default();
        assert_eq!(
            m.point_to_display(&b, Point { row: 1, column: 3 }),
            DisplayPoint { row: 1, column: 3 }
        );
        assert_eq!(
            m.display_to_point(&b, DisplayPoint { row: 1, column: 3 }),
            Point { row: 1, column: 3 }
        );
        assert_eq!(m.wrap_count(&b, 0), 1);
        assert_eq!(m.display_row_count(&b), 2);
    }

    #[test]
    fn column_wrap_splits_rows() {
        let b = buf_with("abcdefghij\nxy");
        let mut m = DisplayMap::default();
        m.soft_wrap = SoftWrap::Column(4);
        assert_eq!(m.wrap_count(&b, 0), 3);
        assert_eq!(m.wrap_count(&b, 1), 1);
        assert_eq!(m.display_row_count(&b), 4);
        // buffer (0,5) → display row 1, col 1
        assert_eq!(
            m.point_to_display(&b, Point { row: 0, column: 5 }),
            DisplayPoint { row: 1, column: 1 }
        );
        // round-trip
        let dp = DisplayPoint { row: 2, column: 1 };
        let p = m.display_to_point(&b, dp);
        assert_eq!(p, Point { row: 0, column: 9 });
        assert_eq!(m.point_to_display(&b, p), dp);
        // row mapping
        assert_eq!(m.buffer_row_of(&b, 0), (0, 0));
        assert_eq!(m.buffer_row_of(&b, 2), (0, 2));
        assert_eq!(m.buffer_row_of(&b, 3), (1, 0));
        // segments slice the expanded line
        assert_eq!(m.display_segment(&b, 0, 0), "abcd");
        assert_eq!(m.display_segment(&b, 0, 2), "ij");
    }

    #[test]
    fn tabs_expand_before_wrap() {
        let b = buf_with("\tab");
        let mut m = DisplayMap {
            tab_width: 4,
            ..Default::default()
        };
        m.soft_wrap = SoftWrap::Column(4);
        // tab (4 cells) + a + b = 6 cols → 2 wraps
        assert_eq!(m.row_width_cols(&b, 0), 6);
        assert_eq!(m.wrap_count(&b, 0), 2);
        // buffer col 1 ('a') is display col 4 → wrap row 1, col 0
        assert_eq!(
            m.point_to_display(&b, Point { row: 0, column: 1 }),
            DisplayPoint { row: 1, column: 0 }
        );
    }

    #[test]
    fn editor_width_needs_wrap_cols() {
        let b = buf_with("abcdefghij");
        let mut m = DisplayMap::default();
        m.soft_wrap = SoftWrap::EditorWidth(500.0);
        // no wrap cols set yet → behaves as off
        assert_eq!(m.wrap_count(&b, 0), 1);
        m.set_wrap_cols(Some(4));
        assert_eq!(m.wrap_count(&b, 0), 3);
        m.set_wrap_cols(None);
        assert_eq!(m.wrap_count(&b, 0), 1);
    }
}
