#![allow(non_snake_case)]
use std::cell::RefCell;
use std::rc::Rc;

use rext_core::Cmd;
use repose_core::prelude::*;
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::textfield::{BasicTextField, TextFieldConfig, TextFieldState};
use repose_ui::*;

use crate::session::SessionRef;

#[derive(Clone)]
struct Item {
    label: String,
    detail: String,
    run: Rc<dyn Fn(&SessionRef)>,
}

pub fn CommandPalette(session: SessionRef) -> View {
    if !*session.palette_open.get() {
        return Box(Modifier::new());
    }
    let th = theme();
    let q = session.palette_query.get().clone();

    let items = build_items(&session, &q);

    let tf = remember_with_key("rext:pal_tf", || {
        RefCell::new(TextFieldState::new())
    });
    // keep TF in sync when opening
    if tf.borrow().text != q {
        tf.borrow_mut().text = q.clone();
    }

    let s_scrim = session.clone();
    let s_tf = session.clone();

    ZStack(Modifier::new().fill_max_size()).child((
        Box(Modifier::new()
            .fill_max_size()
            .background(Color::from_rgba(0, 0, 0, 140))
            .on_click(move || s_scrim.palette_open.set(false))),
        Box(Modifier::new()
            .fill_max_size()
            .align_items(AlignItems::CENTER)
            .justify_content(JustifyContent::FLEX_START)
            .padding(Dp(80.0)))
        .child(
            Box(Modifier::new()
                .width(Dp(560.0))
                .max_height(Dp(420.0))
                .background(th.surface_container_high)
                .border(Dp(1.0), th.outline_variant, Dp(12.0))
                .clip_rounded(Dp(12.0))
                .on_click(|| {}))
            .child(
                Column(Modifier::new().fill_max_width()).child((
                    Box(Modifier::new().fill_max_width().padding(Dp(12.0))).child(
                        BasicTextField(
                            tf.clone(),
                            Modifier::new().fill_max_width().height(Dp(36.0)),
                            "Type a command…",
                            TextFieldConfig {
                                on_change: Some(Rc::new(move |s: String| {
                                    s_tf.palette_query.set(s);
                                })),
                                ..Default::default()
                            },
                        ),
                    ),
                    Box(Modifier::new()
                        .fill_max_width()
                        .height(Dp(1.0))
                        .background(th.outline_variant)),
                    ScrollArea(
                        Modifier::new().fill_max_width().height(Dp(320.0)),
                        remember_scroll_state("rext:pal_scroll"),
                        Column(
                            Modifier::new()
                                .fill_max_width()
                                .padding(Dp(6.0))
                                .gap(Dp(2.0)),
                        )
                        .child(
                            items
                                .into_iter()
                                .enumerate()
                                .map(|(i, it)| row(session.clone(), it, i == 0))
                                .collect::<Vec<_>>(),
                        ),
                    ),
                )),
            ),
        ),
    ))
}

fn row(session: SessionRef, item: Item, hi: bool) -> View {
    let th = theme();
    let run = item.run.clone();
    let s = session.clone();
    Box(Modifier::new()
        .fill_max_width()
        .padding(Dp(10.0))
        .clip_rounded(Dp(8.0))
        .background(if hi {
            th.secondary_container
        } else {
            Color::TRANSPARENT
        })
        .on_click(move || {
            run(&s);
            s.palette_open.set(false);
        }))
    .child(
        Row(Modifier::new()
            .fill_max_width()
            .align_items(AlignItems::CENTER)
            .gap(Dp(12.0)))
        .child((
            Text(item.label)
                .size(Sp(14.0))
                .color(th.on_surface)
                .modifier(Modifier::new().flex_grow(1.0)),
            Text(item.detail)
                .size(Sp(12.0))
                .color(th.on_surface_variant),
        )),
    )
}

fn build_items(session: &SessionRef, q: &str) -> Vec<Item> {
    let mut items: Vec<Item> = vec![
        Item {
            label: "File: Save".into(),
            detail: "Ctrl+S".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Save, false)),
        },
        Item {
            label: "Edit: Undo".into(),
            detail: "Ctrl+Z".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Undo, false)),
        },
        Item {
            label: "Edit: Redo".into(),
            detail: "Ctrl+Shift+Z".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Redo, false)),
        },
        Item {
            label: "Edit: Select All".into(),
            detail: "Ctrl+A".into(),
            run: Rc::new(|s| s.dispatch(Cmd::SelectAll, false)),
        },
        Item {
            label: "File: New Untitled".into(),
            detail: String::new(),
            run: Rc::new(|s| {
                use rext_core::Buffer;
                s.ws.borrow_mut().open_buffer(Buffer::empty());
                s.bump();
            }),
        },
    ];

    {
        let tree = session.tree.borrow();
        for e in tree.iter().filter(|e| !e.is_dir) {
            let path = e.path.clone();
            let name = e.name.clone();
            items.push(Item {
                label: format!("Open: {name}"),
                detail: path.display().to_string(),
                run: Rc::new(move |s| s.open_file(&path)),
            });
        }
    }

    let ql = q.to_lowercase();
    if ql.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|it| {
            it.label.to_lowercase().contains(&ql) || it.detail.to_lowercase().contains(&ql)
        })
        .collect()
}
