#![allow(non_snake_case)]
//! Terminal panel v0: a working command log, not a pty.
//! Input row runs the line via the system shell and appends combined
//! stdout/stderr. `portable-pty` + `vte` replaces the runner later — the
//! scrollback model (`Vec<TermLine>`) stays.

use std::cell::RefCell;
use std::rc::Rc;

use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::textfield::{BasicTextField, TextFieldConfig, TextFieldState};
use repose_ui::*;
use crate::session::SessionRef;

material_symbols! {
    terminal: '\u{EB8E}',
    chevron_right: '\u{E5CC}',
}

#[derive(Clone)]
struct TermLine {
    text: String,
    kind: LineKind,
}

#[derive(Clone, Copy, PartialEq)]
enum LineKind {
    Echo,
    Out,
    Err,
}

pub fn TerminalPanel(session: SessionRef) -> View {
    let th = theme();
    let log: Rc<RefCell<Vec<TermLine>>> = remember_with_key("rext:term_log", || {
        RefCell::new(vec![TermLine {
            text: "rext shell — type a command, Enter to run (pty later)".into(),
            kind: LineKind::Err,
        }])
    });
    let tf = remember_with_key("rext:term_tf", || {
        RefCell::new(TextFieldState::new())
    });

    let rows: Vec<View> = log
        .borrow()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let color = match l.kind {
                LineKind::Echo => th.primary,
                LineKind::Out => th.on_surface,
                LineKind::Err => th.on_surface_variant,
            };
            Row(Modifier::new()
                .key(i as u64)
                .fill_max_width()
                .padding(Dp(4.0)))
            .child(
                Text(l.text.clone())
                    .size(Sp(12.0))
                    .color(color),
            )
        })
        .collect();

    let log_run = log.clone();
    let sess = session.clone();
    let cwd = sess.ws.borrow().root.clone();

    Column(Modifier::new()
        .fill_max_size()
        .background(th.surface_container_lowest))
    .child((
        Row(Modifier::new()
            .fill_max_width()
            .height(Dp(28.0))
            .padding(Dp(10.0))
            .align_items(AlignItems::CENTER)
            .gap(Dp(6.0)))
        .child((
            Icon(Symbols::terminal)
                .size(Sp(14.0))
                .color(th.on_surface_variant),
            Text("TERMINAL".to_string())
                .size(Sp(10.5))
                .color(th.on_surface_variant),
            Spacer(),
            Text(cwd.display().to_string())
                .size(Sp(10.5))
                .color(th.on_surface_variant),
        )),
        ScrollArea(
            Modifier::new().fill_max_width().flex_grow(1.0),
            remember_scroll_state("rext:term_scroll"),
            Column(Modifier::new().fill_max_width().padding(Dp(8.0))).child(rows),
        ),
        Row(Modifier::new()
            .fill_max_width()
            .height(Dp(32.0))
            .padding(Dp(8.0))
            .gap(Dp(6.0))
            .align_items(AlignItems::CENTER)
            .background(th.surface_container))
        .child((
            Icon(Symbols::chevron_right)
                .size(Sp(14.0))
                .color(th.primary),
            BasicTextField(
                tf.clone(),
                Modifier::new().fill_max_width(),
                "$ command…",
                TextFieldConfig {
                    on_submit: Some(Rc::new(move |cmd: String| {
                        run_command(&log_run, &cwd, &cmd);
                    })),
                    line_limits: repose_core::TextFieldLineLimits::SingleLine,
                    ..Default::default()
                },
            ),
        )),
    ))
}

fn run_command(log: &Rc<RefCell<Vec<TermLine>>>, cwd: &std::path::Path, cmd: &str) {
    let cmd = cmd.trim().to_string();
    if cmd.is_empty() {
        return;
    }
    log.borrow_mut().push(TermLine {
        text: format!("$ {cmd}"),
        kind: LineKind::Echo,
    });
    if cmd == "clear" {
        log.borrow_mut().clear();
        return;
    }
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(cwd)
        .output()
    {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            let err = String::from_utf8_lossy(&out.stderr);
            if !err.is_empty() {
                text.push_str(&err);
            }
            if !out.status.success() {
                text.push_str(&format!("[exit {}]", out.status));
            }
            let text = text.trim_end().to_string();
            log.borrow_mut().push(TermLine {
                text: if text.is_empty() {
                    "(no output)".into()
                } else {
                    text.chars().take(4000).collect()
                },
                kind: LineKind::Out,
            });
        }
        Err(e) => log.borrow_mut().push(TermLine {
            text: format!("spawn failed: {e}"),
            kind: LineKind::Err,
        }),
    }
    // Cap scrollback.
    let mut l = log.borrow_mut();
    if l.len() > 500 {
        let excess = l.len() - 500;
        l.drain(..excess);
    }
}
