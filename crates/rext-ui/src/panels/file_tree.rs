#![allow(non_snake_case)]
use repose_core::prelude::*;
use repose_core::modifier::PaddingValues;
use repose_material::{Icon, material_symbols};
use repose_ui::scroll::{ScrollArea, remember_scroll_state};
use repose_ui::*;
use crate::session::SessionRef;

material_symbols! {
    folder: '\u{E2C7}',
    folder_open: '\u{E2C8}',
    description: '\u{E873}',
    chevron_right: '\u{E5CC}',
    expand_more: '\u{E5CF}',
    refresh: '\u{E5D5}',
    unfold_less: '\u{E5D6}',
    my_location: '\u{E55C}',
}

pub fn FileTreePanel(session: SessionRef) -> View {
    let th = theme();
    let (root_name, entries) = {
        let ws = session.ws.borrow();
        let name = ws
            .root
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        (name, session.tree.borrow().clone())
    };

    let rows: Vec<View> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let s = session.clone();
            let path = e.path.clone();
            let is_dir = e.is_dir;
            let indent = 8.0 + e.depth as f32 * 14.0;
            let icon = if is_dir {
                if e.expanded {
                    Symbols::folder_open
                } else {
                    Symbols::folder
                }
            } else {
                Symbols::description
            };
            let chev = if is_dir {
                Some(if e.expanded {
                    Symbols::expand_more
                } else {
                    Symbols::chevron_right
                })
            } else {
                None
            };

            Row(Modifier::new()
                .key(i as u64)
                .fill_max_width()
                .height(Dp(24.0))
                .padding_values(PaddingValues {
                    left: Dp(indent),
                    right: Dp(8.0),
                    top: Dp(0.0),
                    bottom: Dp(0.0),
                })
                .align_items(AlignItems::CENTER)
                .gap(Dp(4.0))
                .on_click(move || {
                    if is_dir {
                        s.toggle_dir(i);
                    } else {
                        s.open_file(&path);
                    }
                }))
            .child((
                match chev {
                    Some(c) => Icon(c).size(Sp(14.0)).color(th.on_surface_variant),
                    None => Box(Modifier::new().width(Dp(14.0))),
                },
                Icon(icon).size(Sp(15.0)).color(if is_dir {
                    th.primary
                } else {
                    th.on_surface_variant
                }),
                Text(e.name.clone())
                    .size(Sp(12.5))
                    .color(th.on_surface),
            ))
        })
        .collect();

    Column(
        Modifier::new()
            .fill_max_size()
            .background(th.surface_container_low),
    )
    .child((
        Row(Modifier::new()
            .fill_max_width()
            .height(Dp(28.0))
            .padding(Dp(10.0))
            .gap(Dp(4.0))
            .align_items(AlignItems::CENTER))
        .child((
            Text(root_name.to_uppercase())
                .size(Sp(10.5))
                .color(th.on_surface_variant),
            Spacer(),
            {
                let s = session.clone();
                Box(Modifier::new()
                    .size(Dp(20.0), Dp(20.0))
                    .clip_rounded(Dp(4.0))
                    .on_click(move || s.refresh_tree()))
                .child(
                    Icon(Symbols::refresh)
                        .size(Sp(14.0))
                        .color(th.on_surface_variant),
                )
            },
            {
                let s = session.clone();
                Box(Modifier::new()
                    .size(Dp(20.0), Dp(20.0))
                    .clip_rounded(Dp(4.0))
                    .on_click(move || s.collapse_all()))
                .child(
                    Icon(Symbols::unfold_less)
                        .size(Sp(14.0))
                        .color(th.on_surface_variant),
                )
            },
            {
                let s = session.clone();
                Box(Modifier::new()
                    .size(Dp(20.0), Dp(20.0))
                    .clip_rounded(Dp(4.0))
                    .on_click(move || s.reveal_active()))
                .child(
                    Icon(Symbols::my_location)
                        .size(Sp(14.0))
                        .color(th.on_surface_variant),
                )
            },
        )),
        ScrollArea(
            Modifier::new().fill_max_size(),
            remember_scroll_state("rext:tree"),
            Column(Modifier::new().fill_max_width()).child(rows),
        ),
    ))
}
