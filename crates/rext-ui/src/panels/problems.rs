#![allow(non_snake_case)]
//! Problems panel: real buffer diagnostics available today (empty-file,
//! no-path, line-length, trailing-whitespace, tab indentation warnings).
//! LSP diagnostics plug into `Session::diagnostics` later — the list shape
//! stays the same.

use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;
use crate::session::SessionRef;

material_symbols! {
    error: '\u{E000}',
    warning: '\u{E002}',
    check_circle: '\u{E86C}',
    description: '\u{E873}',
}

#[derive(Clone)]
struct Diagnostic {
    severity: Severity,
    message: String,
    location: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Severity {
    Error,
    Warning,
}

fn collect(session: &SessionRef) -> Vec<Diagnostic> {
    let ws = session.ws.borrow();
    let mut out = Vec::new();
    for id in &ws.tab_order {
        let Some(h) = ws.buffers.get(id) else {
            continue;
        };
        let title = h.buffer.title();
        if h.buffer.path().is_none() {
            out.push(Diagnostic {
                severity: Severity::Warning,
                message: "buffer has no path — save will fail until Save As".into(),
                location: format!("{title}:1"),
            });
        }
        let mut long_lines = 0u32;
        let mut trailing_ws = 0u32;
        let mut tab_indent = 0u32;
        for row in 0..h.buffer.len_lines() {
            let line = h.buffer.line(row);
            let n_chars = line.chars().count();
            if n_chars > 120 {
                long_lines += 1;
            }
            if line.ends_with(' ') || line.ends_with('\t') {
                trailing_ws += 1;
            }
            if line.starts_with('\t') {
                tab_indent += 1;
            }
            if long_lines + trailing_ws + tab_indent > 50 {
                break; // cap scan for huge files
            }
        }
        if long_lines > 0 {
            out.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("{long_lines} lines exceed 120 chars"),
                location: title.clone(),
            });
        }
        if trailing_ws > 0 {
            out.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("{trailing_ws} lines have trailing whitespace"),
                location: title.clone(),
            });
        }
        if tab_indent > 0 {
            out.push(Diagnostic {
                severity: Severity::Warning,
                message: format!("{tab_indent} lines use tab indentation"),
                location: title.clone(),
            });
        }
        let _ = id;
    }
    out
}

pub fn ProblemsPanel(session: SessionRef) -> View {
    let th = theme();
    let diags = collect(&session);

    let header = Row(Modifier::new()
        .fill_max_width()
        .height(Dp(28.0))
        .padding(Dp(10.0))
        .align_items(AlignItems::CENTER)
        .gap(Dp(8.0)))
    .child((
        Text(format!("PROBLEMS ({})", diags.len()))
            .size(Sp(10.5))
            .color(th.on_surface_variant),
        Spacer(),
        Text("buffer lints · LSP later".to_string())
            .size(Sp(10.5))
            .color(th.on_surface_variant),
    ));

    let body: View = if diags.is_empty() {
        Row(Modifier::new()
            .fill_max_width()
            .padding(Dp(12.0))
            .gap(Dp(8.0))
            .align_items(AlignItems::CENTER))
        .child((
            Icon(Symbols::check_circle)
                .size(Sp(16.0))
                .color(th.primary),
            Text("No problems detected.".to_string())
                .size(Sp(12.5))
                .color(th.on_surface_variant),
        ))
    } else {
        let rows: Vec<View> = diags
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                let (icon, color) = match d.severity {
                    Severity::Error => (Symbols::error, th.error),
                    Severity::Warning => (Symbols::warning, th.tertiary),
                };
                Row(Modifier::new()
                    .key(i as u64)
                    .fill_max_width()
                    .padding(Dp(8.0))
                    .gap(Dp(8.0))
                    .align_items(AlignItems::CENTER))
                .child((
                    Icon(icon).size(Sp(14.0)).color(color),
                    Text(d.message).size(Sp(12.5)).color(th.on_surface),
                    Spacer(),
                    Text(d.location).size(Sp(11.0)).color(th.on_surface_variant),
                ))
            })
            .collect();
        ScrollArea(
            Modifier::new().fill_max_size(),
            remember_scroll_state("rext:problems"),
            Column(Modifier::new().fill_max_width()).child(rows),
        )
    };

    Column(Modifier::new()
        .fill_max_size()
        .background(th.surface_container_low))
    .child((header, body))
}
