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
    let s_sub = session.clone();
    let s_up = session.clone();
    let s_dn = session.clone();
    // Clamp selection into range each compose.
    let sel = (*session.palette_sel.get()).min(items.len().saturating_sub(1));

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
                    Box(Modifier::new()
                        .fill_max_width()
                        .padding(Dp(12.0))
                        // ArrowUp/Down move selection, Enter runs it.
                        .on_key_event(move |ev: KeyEvent| {
                            use repose_core::input::{Key, KeyEventType};
                            if ev.event_type != KeyEventType::Down {
                                return false;
                            }
                            match ev.key {
                                Key::ArrowDown => {
                                    let n = build_items(&s_dn, &s_dn.palette_query.get()).len();
                                    s_dn.palette_sel.update(|v| {
                                        *v = (*v + 1).min(n.saturating_sub(1))
                                    });
                                    true
                                }
                                Key::ArrowUp => {
                                    s_up.palette_sel.update(|v| {
                                        *v = v.saturating_sub(1)
                                    });
                                    true
                                }
                                _ => false,
                            }
                        }))
                    .child(
                        BasicTextField(
                            tf.clone(),
                            Modifier::new().fill_max_width().height(Dp(36.0)),
                            "Type a command…",
                            TextFieldConfig {
                                on_change: Some(Rc::new(move |s: String| {
                                    s_tf.palette_query.set(s);
                                    s_tf.palette_sel.set(0);
                                })),
                                on_submit: Some(Rc::new(move |_: String| {
                                    let items =
                                        build_items(&s_sub, &s_sub.palette_query.get());
                                    let i = (*s_sub.palette_sel.get())
                                        .min(items.len().saturating_sub(1));
                                    if let Some(it) = items.into_iter().nth(i) {
                                        s_sub.record_command(&it.label);
                                        (it.run)(&s_sub);
                                        s_sub.palette_open.set(false);
                                    }
                                })),
                                line_limits:
                                    repose_core::TextFieldLineLimits::SingleLine,
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
                                .map(|(i, it)| row(session.clone(), it, i == sel))
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
    let label = item.label.clone();
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
            s.record_command(&label);
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
            label: "Edit: Copy".into(),
            detail: "Ctrl+C".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Copy, false)),
        },
        Item {
            label: "Edit: Cut".into(),
            detail: "Ctrl+X".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Cut, false)),
        },
        Item {
            label: "Edit: Paste".into(),
            detail: "Ctrl+V".into(),
            run: Rc::new(|s| s.dispatch(Cmd::Paste, false)),
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
        Item {
            label: "File: Close Buffer".into(),
            detail: "closes active tab".into(),
            run: Rc::new(|s| {
                let idx = s.ws.borrow().active;
                s.ws.borrow_mut().close_tab(idx);
                s.bump();
            }),
        },
        Item {
            label: "View: Refresh Explorer".into(),
            detail: String::new(),
            run: Rc::new(|s| s.refresh_tree()),
        },
        Item {
            label: "View: Reveal Active File".into(),
            detail: String::new(),
            run: Rc::new(|s| s.reveal_active()),
        },
        Item {
            label: "Tasks: Refresh".into(),
            detail: "reload .vscode/tasks.json".into(),
            run: Rc::new(|s| {
                s.refresh_tasks();
                s.ws.borrow_mut().status = "tasks refreshed".into();
                s.bump();
            }),
        },
    ];

    {
        let cx = session.task_context();
        let root = session.ws.borrow().root.clone();
        for (i, t) in session.tasks.borrow().templates.iter().enumerate() {
            let resolved = t.resolve(&root, &cx);
            items.push(Item {
                label: format!("Tasks: {}", resolved.resolved_label),
                detail: resolved.command_label,
                run: Rc::new(move |s| s.schedule_task(i)),
            });
        }
    }
    for t in session.tasks.borrow().last_scheduled.clone() {
        let label = format!("Tasks: Rerun {}", t.resolved_label);
        let detail = t.command_label.clone();
        items.push(Item {
            label,
            detail,
            run: Rc::new(move |s| {
                s.pending_spawns.borrow_mut().push(t.clone());
                s.bump();
            }),
        });
    }

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

    if q.chars().all(char::is_whitespace) {
        return with_recents_first(session, items);
    }
    let candidates: Vec<crate::fuzzy::StringMatchCandidate> = items
        .iter()
        .enumerate()
        .map(|(i, it)| crate::fuzzy::StringMatchCandidate::new(i, humanized(&it.label)))
        .collect();
    let matches = crate::fuzzy::match_strings(q, &candidates, true);
    let mut out: Vec<Item> = matches
        .into_iter()
        .filter_map(|m| items.get(m.candidate_id).cloned())
        .collect();
    if out.is_empty() {
        out = items;
    }
    with_recents_first(session, out)
}

fn humanized(label: &str) -> String {
    label.replace('_', " ").to_lowercase()
}

fn with_recents_first(session: &SessionRef, mut items: Vec<Item>) -> Vec<Item> {
    let recents = session.recent_commands();
    if recents.is_empty() {
        return items;
    }
    let mut rest = Vec::with_capacity(items.len());
    let mut head = Vec::new();
    for it in items.drain(..) {
        if recents.iter().any(|r| *r == it.label) {
            head.push(it);
        } else {
            rest.push(it);
        }
    }
    head.extend(rest);
    head
}
