//! Buffer → display rows. Soft-wrap stub (wrap width in px later).
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
    /// Wrap at this pixel width (Phase 2: drive with metrics).
    EditorWidth(f32),
}

pub struct DisplayMap {
    pub soft_wrap: SoftWrap,
    pub tab_width: usize,
}

impl Default for DisplayMap {
    fn default() -> Self {
        Self {
            soft_wrap: SoftWrap::Off,
            tab_width: 4,
        }
    }
}

impl DisplayMap {
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

    pub fn point_to_display(&self, buf: &Buffer, p: Point) -> DisplayPoint {
        // Soft-wrap off: 1:1 rows; column accounts for tabs before p.column.
        let line = buf.line(p.row);
        let mut col = 0usize;
        for (i, ch) in line.chars().enumerate() {
            if i >= p.column {
                break;
            }
            if ch == '\t' {
                col += self.tab_width - (col % self.tab_width);
            } else {
                col += 1;
            }
        }
        DisplayPoint {
            row: p.row,
            column: col,
        }
    }

    pub fn display_to_point(&self, buf: &Buffer, dp: DisplayPoint) -> Point {
        let line = buf.line(dp.row);
        let mut dcol = 0usize;
        let mut col = 0usize;
        for ch in line.chars() {
            let w = if ch == '\t' {
                self.tab_width - (dcol % self.tab_width)
            } else {
                1
            };
            if dcol + w > dp.column {
                break;
            }
            dcol += w;
            col += 1;
        }
        Point {
            row: dp.row.min(buf.len_lines().saturating_sub(1)),
            column: col.min(buf.line_len(dp.row)),
        }
    }
}
