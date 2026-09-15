#![allow(non_snake_case)]

use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;
use rext_core::{Diagnostic, DiagnosticEntry, Point, Severity};
use crate::session::SessionRef;

material_symbols! {
    error: '\u{E000}',
    warning: '\u{E002}',
    info: '\u{E88E}',
    check_circle: '\u{E86C}',
}

pub fn ProblemsPanel(session: SessionRef) -> View {
    let th = theme();
    session.refresh_lints();
    let items = session.problem_items();
    let (errors, warnings) = session.problem_counts();

    let header = Row(Modifier::new()
        .fill_max_width()
        .height(Dp(28.0))
        .padding(Dp(10.0))
        .align_items(AlignItems::CENTER)
        .gap(Dp(8.0)))
    .child((
        Text(format!("PROBLEMS {} errors, {} warnings", errors, warnings))
            .size(Sp(10.5))
            .color(th.on_surface_variant),
        Spacer(),
        Text("buffer lints · LSP next".to_string())
            .size(Sp(10.5))
            .color(th.on_surface_variant),
    ));

    let body: View = if items.is_empty() {
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
        let rows: Vec<View> = items
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                let (icon, color) = match d.severity {
                    Severity::Error => (Symbols::error, th.error),
                    Severity::Warning => (Symbols::warning, th.tertiary),
                    _ => (Symbols::info, th.on_surface_variant),
                };
                let s = session.clone();
                let target = d.target;
                Row(Modifier::new()
                    .key(i as u64)
                    .fill_max_width()
                    .padding(Dp(8.0))
                    .gap(Dp(8.0))
                    .align_items(AlignItems::CENTER)
                    .on_click(move || {
                        if let Some(t) = target {
                            s.jump_to(t.0, t.1);
                        }
                    }))
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

#[derive(Clone)]
pub struct ProblemItem {
    pub severity: Severity,
    pub message: String,
    pub location: String,
    pub target: Option<(rext_core::BufferId, Point)>,
}

pub fn lint_buffer(
    id: rext_core::BufferId,
    buf: &rext_core::Buffer,
) -> Vec<DiagnosticEntry> {
    let mut out = Vec::new();
    let title_unsaved = buf.path().is_none();
    if title_unsaved {
        out.push(DiagnosticEntry::new(
            Point::default()..Point::default(),
            Diagnostic {
                source: Some("rext-lint".into()),
                severity: Severity::Warning,
                message: "buffer has no path — save will fail until Save As".into(),
                ..Default::default()
            },
        ));
    }
    let mut scanned = 0usize;
    for row in 0..buf.len_lines() {
        let line = buf.line(row);
        let n_chars = line.chars().count();
        if n_chars > 120 {
            out.push(DiagnosticEntry::new(
                Point { row, column: 120 }..Point { row, column: n_chars },
                Diagnostic {
                    source: Some("rext-lint".into()),
                    severity: Severity::Warning,
                    message: format!("line exceeds 120 chars ({n_chars})"),
                    ..Default::default()
                },
            ));
        }
        if line.ends_with(' ') || line.ends_with('\t') {
            out.push(DiagnosticEntry::new(
                Point { row, column: n_chars.saturating_sub(1) }..Point { row, column: n_chars },
                Diagnostic {
                    source: Some("rext-lint".into()),
                    severity: Severity::Warning,
                    message: "trailing whitespace".into(),
                    ..Default::default()
                },
            ));
        }
        if line.starts_with('\t') {
            out.push(DiagnosticEntry::new(
                Point { row, column: 0 }..Point { row, column: 1 },
                Diagnostic {
                    source: Some("rext-lint".into()),
                    severity: Severity::Info,
                    message: "tab indentation".into(),
                    ..Default::default()
                },
            ));
        }
        scanned += 1;
        if scanned > 5000 || out.len() > 200 {
            break;
        }
    }
    let _ = id;
    out
}
