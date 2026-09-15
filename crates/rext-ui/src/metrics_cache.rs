//! Line metrics via repose_text — same font path as Canvas draw_text (family None).

use repose_text::{TextMetrics, metrics_for_textfield};
use std::collections::HashMap;

const FONT_WEIGHT: u16 = 400;
const FONT_STYLE: u8 = 0;

#[derive(Default)]
pub struct LineMetricsCache {
    /// (buffer_version, row) → metrics for display line string
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
                None,
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
        // positions are per grapheme boundary; map to char col ≈ index
        best.min(m.positions.len().saturating_sub(1))
    }

    pub fn x_for_col(m: &TextMetrics, col: usize) -> f32 {
        m.positions
            .get(col)
            .copied()
            .or_else(|| m.positions.last().copied())
            .unwrap_or(0.0)
    }
}
