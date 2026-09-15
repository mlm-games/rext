#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::textfield::{BasicTextField, TextFieldConfig, TextFieldState};
use repose_ui::*;
use rext_core::{Point, SearchHit, SearchOptions, SearchQuery};

use crate::session::SessionRef;

material_symbols! {
    search: '\u{E8B6}',
    description: '\u{E873}',
}

pub fn SearchPanel(session: SessionRef) -> View {
    let th = theme();
    let q = session.search_query.get().clone();

    let tf = remember_with_key("rext:search_tf", || {
        RefCell::new(TextFieldState::new())
    });
    if tf.borrow().text != q {
        tf.borrow_mut().text = q.clone();
    }

    let hits = run_search(&session);
    let hit_count = hits.len();

    let s_tf = session.clone();
    let s_case = session.clone();
    let s_word = session.clone();
    let case_on = *session.search_case.get();
    let word_on = *session.search_word.get();

    let rows: Vec<View> = hits
        .into_iter()
        .enumerate()
        .take(200)
        .map(|(i, h)| hit_row(session.clone(), h, i))
        .collect();

    Column(
        Modifier::new()
            .fill_max_size()
            .background(th.surface_container_low),
    )
    .child((
        Row(Modifier::new()
            .fill_max_width()
            .padding(Dp(8.0))
            .gap(Dp(6.0))
            .align_items(AlignItems::CENTER))
        .child((
            Icon(Symbols::search)
                .size(Sp(15.0))
                .color(th.on_surface_variant),
            BasicTextField(
                tf.clone(),
                Modifier::new().flex_grow(1.0).height(Dp(30.0)),
                "Search project…",
                TextFieldConfig {
                    on_change: Some(Rc::new(move |s: String| {
                        s_tf.search_query.set(s);
                    })),
                    on_submit: Some(Rc::new(move |_: String| {})),
                    line_limits: repose_core::TextFieldLineLimits::SingleLine,
                    ..Default::default()
                },
            ),
            toggle_chip(s_case, "Aa", case_on, "case sensitive"),
            toggle_chip(s_word, r#""…""#, word_on, "whole word"),
            Text(if q.is_empty() {
                String::new()
            } else {
                format!("{} hits", hit_count)
            })
            .size(Sp(11.0))
            .color(th.on_surface_variant),
        )),
        ScrollArea(
            Modifier::new().fill_max_size(),
            remember_scroll_state("rext:search_scroll"),
            Column(Modifier::new().fill_max_width().padding(Dp(6.0)).gap(Dp(2.0)))
                .child(rows),
        ),
    ))
}

fn toggle_chip(session: SessionRef, label: &str, on: bool, hint: &str) -> View {
    let th = theme();
    let s = session.clone();
    let is_case = label == "Aa";
    Box(Modifier::new()
        .padding(Dp(6.0))
        .clip_rounded(Dp(4.0))
        .background(if on {
            th.secondary_container
        } else {
            Color::TRANSPARENT
        })
        .border(Dp(1.0), th.outline_variant, Dp(4.0))
        .on_click(move || {
            if is_case {
                s.search_case.update(|v| *v = !*v);
            } else {
                s.search_word.update(|v| *v = !*v);
            }
        }))
    .child(
        Text(format!("{label} {hint}"))
            .size(Sp(10.0))
            .color(if on { th.on_surface } else { th.on_surface_variant }),
    )
}

fn hit_row(session: SessionRef, hit: SearchHit, i: usize) -> View {
    let th = theme();
    let path = hit.path.clone();
    let row = hit.row;
    let rel = path
        .strip_prefix(&session.ws.borrow().root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    let preview = hit.preview.clone();
    let s = session.clone();
    Row(Modifier::new()
        .key(i as u64)
        .fill_max_width()
        .padding(Dp(6.0))
        .gap(Dp(8.0))
        .align_items(AlignItems::CENTER)
        .clip_rounded(Dp(4.0))
        .on_click(move || {
            let _ = s.ws.borrow_mut().open_path(&path);
            s.bump();
            s.jump_to_active(Point { row, column: 0 });
        }))
    .child((
        Icon(Symbols::description)
            .size(Sp(13.0))
            .color(th.on_surface_variant),
        Column(Modifier::new().flex_grow(1.0)).child((
            Text(rel).size(Sp(11.5)).color(th.on_surface),
            Text(format!("{}: {}", row + 1, preview))
                .size(Sp(11.0))
                .color(th.on_surface_variant),
        )),
    ))
}

pub fn run_search(session: &SessionRef) -> Vec<SearchHit> {
    let q = session.search_query.get().clone();
    if q.is_empty() {
        return Vec::new();
    }
    let opts = SearchOptions {
        case_sensitive: *session.search_case.get(),
        whole_word: *session.search_word.get(),
        ..Default::default()
    };
    let Ok(query) = SearchQuery::text(&q, opts) else {
        return Vec::new();
    };
    let ws = session.ws.borrow();
    let open: Vec<(std::path::PathBuf, String)> = ws
        .buffers
        .values()
        .filter_map(|h| {
            h.buffer.path().map(|p| {
                (
                    p.to_path_buf(),
                    (0..h.buffer.len_lines())
                        .map(|r| h.buffer.line(r))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })
        })
        .collect();
    let root = ws.root.clone();
    drop(ws);
    rext_core::search_project(&root, &query, &open)
}
