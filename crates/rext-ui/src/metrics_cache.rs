//! Line metrics via repose_text — same font path as Canvas code text
//! (`font_family = Some("monospace")` → JetBrains Mono when the
//! `repose-text/monospace` feature is enabled, system monospace otherwise).

use repose_text::{TextMetrics, metrics_for_textfield};
use std::collections::HashMap;

/// Font family for all code surface text. Must match the family passed to
/// `draw_text_with_family` in the surface paint — metrics and paint go
/// through the same parley stack, so hit-test and glyphs agree.
pub const CODE_FAMILY: Option<&str> = Some("monospace");

const FONT_WEIGHT: u16 = 400;
const FONT_STYLE: u8 = 0;

#[derive(Default)]
pub struct LineMetricsCache {
    /// (buffer_version, row) -> metrics for display line string
    map: HashMap<(u64, usize), (String, TextMetrics)>,
}

impl LineMetricsCache {
    pub fn metrics(
        &mut self,
        version: u64,
        row: usize,
        display_line: &str,
        font_px: f32,
    ) -> &TextMetrics {
        let key = (version, row);
        let needs = match self.map.get(&key) {
            Some((s, _)) if s == display_line => false,
            _ => true,
        };
        if needs {
            let m = metrics_for_textfield(
                display_line,
                font_px,
                CODE_FAMILY,
                FONT_WEIGHT,
                FONT_STYLE,
                0.0,
                None,
            );
            self.map.insert(key, (display_line.to_string(), m));
        }
        &self.map.get(&key).unwrap().1
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Nearest grapheme boundary for x in line-local px.
    /// Returns a display column in `0..=graphemes`: the index of the nearest
    /// leading edge in `positions` (which has graphemes+1 entries). The last
    /// entry is the trailing edge of the final grapheme, so a click past EOL
    /// correctly yields `graphemes` instead of `graphemes-1`.
    pub fn col_for_x(m: &TextMetrics, x: f32) -> usize {
        if m.positions.is_empty() {
            return 0;
        }
        let mut best = 0usize;
        let mut best_d = f32::INFINITY;
        for (i, &px) in m.positions.iter().enumerate() {
            let d = (px - x).abs();
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        best
    }

    /// X for the leading edge of grapheme column `col`.
    /// `positions` has graphemes+1 entries: `positions[col]` is the leading
    /// edge of grapheme `col`, `positions.last()` the trailing edge of the
    /// last grapheme. Do NOT clamp inward — clamping to `len-1` drops the
    /// last char from selection rects and caret placement.
    pub fn x_for_col(m: &TextMetrics, col: usize) -> f32 {
        m.positions.get(col).copied().unwrap_or_else(|| {
            // col past end (shouldn't happen on a well-formed line): fall back
            // to the line's trailing edge so the caret stays visible.
            m.positions.last().copied().unwrap_or(0.0)
        })
    }
}
