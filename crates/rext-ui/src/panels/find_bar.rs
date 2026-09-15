#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::textfield::{BasicTextField, TextFieldConfig, TextFieldState};
use repose_ui::*;
use rext_core::{Point, SearchQuery};

use crate::session::SessionRef;

material_symbols! {
    search: '\u{E8B6}',
    close: '\u{E5CD}',
    keyboard_arrow_up: '\u{E316}',
    keyboard_arrow_down: '\u{E313}',
}

pub fn FindBar(session: SessionRef) -> View {
    if !*session.find_open.get() {
        return Box(Modifier::new());
    }
    let th = theme();
    let q = session.find_query.get().clone();

    let tf = remember_with_key("rext:find_tf", || {
        RefCell::new(TextFieldState::new())
    });
    if tf.borrow().text != q {
        tf.borrow_mut().text = q.clone();
    }

    let (match_total, _) = find_matches(&session);
    let match_idx = (*session.find_match_idx.get()).min(match_total.saturating_sub(1));
    let count_text = if q.is_empty() {
        String::new()
    } else if match_total == 0 {
        "0/0".to_string()
    } else {
        format!("{}/{}", match_idx + 1, match_total)
    };

    let s_close = session.clone();
    let s_prev = session.clone();
    let s_next = session.clone();
    let s_tf = session.clone();
    let s_sub = session.clone();

    Box(Modifier::new()
        .fill_max_width()
        .background(th.surface_container_high)
        .border(Dp(1.0), th.outline_variant, Dp(8.0))
        .clip_rounded(Dp(8.0))
        .padding(Dp(8.0)))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .gap(Dp(6.0))
            .align_items(AlignItems::CENTER))
        .child((
            Icon(Symbols::search)
                .size(Sp(15.0))
                .color(th.on_surface_variant),
            BasicTextField(
                tf.clone(),
                Modifier::new().flex_grow(1.0).height(Dp(30.0)),
                "Find in buffer…",
                TextFieldConfig {
                    on_change: Some(Rc::new(move |s: String| {
                        s_tf.find_query.set(s);
                        s_tf.find_match_idx.set(0);
                    })),
                    on_submit: Some(Rc::new(move |_: String| {
                        s_sub.find_match_idx.update(|v| {
                            let (n, _) = find_matches(&s_sub);
                            *v = if n == 0 { 0 } else { (*v + 1) % n };
                        });
                        jump_to_match(&s_sub);
                    })),
                    line_limits: repose_core::TextFieldLineLimits::SingleLine,
                    ..Default::default()
                },
            ),
            Text(count_text)
                .size(Sp(11.0))
                .color(th.on_surface_variant),
            Box(Modifier::new()
                .size(Dp(22.0), Dp(22.0))
                .on_click(move || {
                    s_prev.find_match_idx.update(|v| {
                        let (n, _) = find_matches(&s_prev);
                        *v = if n == 0 { 0 } else { (*v + n - 1) % n };
                    });
                    jump_to_match(&s_prev);
                }))
            .child(
                Icon(Symbols::keyboard_arrow_up)
                    .size(Sp(15.0))
                    .color(th.on_surface_variant),
            ),
            Box(Modifier::new()
                .size(Dp(22.0), Dp(22.0))
                .on_click(move || {
                    s_next.find_match_idx.update(|v| {
                        let (n, _) = find_matches(&s_next);
                        *v = if n == 0 { 0 } else { (*v + 1) % n };
                    });
                    jump_to_match(&s_next);
                }))
            .child(
                Icon(Symbols::keyboard_arrow_down)
                    .size(Sp(15.0))
                    .color(th.on_surface_variant),
            ),
            Box(Modifier::new()
                .size(Dp(22.0), Dp(22.0))
                .on_click(move || s_close.find_open.set(false)))
            .child(
                Icon(Symbols::close)
                    .size(Sp(14.0))
                    .color(th.on_surface_variant),
            ),
        )),
    )
}

pub fn find_matches(session: &SessionRef) -> (usize, Vec<(usize, usize, usize)>) {
    let q = session.find_query.get().clone();
    if q.is_empty() {
        return (0, Vec::new());
    }
    let ws = session.ws.borrow();
    let Some(h) = ws.active() else {
        return (0, Vec::new());
    };
    let Ok(query) = SearchQuery::text(&q, Default::default()) else {
        return (0, Vec::new());
    };
    let mut out = Vec::new();
    for row in 0..h.buffer.len_lines() {
        let line = h.buffer.line(row);
        for r in query.find_in_line(&line) {
            // Convert byte range to char cols.
            let to_col = |b: usize| line.as_bytes()[..b.min(line.len())].iter().filter(|&&c| (c & 0xC0) != 0x80).count();
            out.push((row, to_col(r.start), to_col(r.end)));
        }
    }
    (out.len(), out)
}

fn jump_to_match(session: &SessionRef) {
    let (_, matches) = find_matches(session);
    if matches.is_empty() {
        return;
    }
    let idx = (*session.find_match_idx.get()).min(matches.len() - 1);
    let (row, sc, _) = matches[idx];
    session.jump_to_active(Point { row, column: sc });
}
