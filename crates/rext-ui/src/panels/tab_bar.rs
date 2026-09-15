#![allow(non_snake_case)]
use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::*;
use crate::chrome::layout;
use crate::session::SessionRef;

material_symbols! {
    close: '\u{E5CD}',
    add: '\u{E145}',
}

pub fn TabBar(session: SessionRef) -> View {
    let th = theme();
    let tabs: Vec<(String, bool, bool)> = {
        let ws = session.ws.borrow();
        ws.tab_order
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let h = ws.buffers.get(id)?;
                Some((
                    h.buffer.title(),
                    h.buffer.dirty(),
                    i == ws.active,
                ))
            })
            .collect()
    };

    let items: Vec<View> = tabs
        .into_iter()
        .enumerate()
        .map(|(i, (title, dirty, active))| {
            let s_sel = session.clone();
            let s_close = session.clone();
            Row(Modifier::new()
                .key(i as u64)
                .height(Dp(layout::TAB_H))
                .padding(Dp(12.0))
                .gap(Dp(8.0))
                .align_items(AlignItems::CENTER)
                .background(if active {
                    th.surface_container_lowest
                } else {
                    th.surface_container
                })
                .on_click(move || {
                    s_sel.ws.borrow_mut().select_tab(i);
                    s_sel.bump();
                }))
            .child((
                Text(title)
                    .size(Sp(12.5))
                    .color(if active {
                        th.on_surface
                    } else {
                        th.on_surface_variant
                    }),
                if dirty {
                    Box(Modifier::new()
                        .size(Dp(7.0), Dp(7.0))
                        .background(th.on_surface_variant)
                        .clip_rounded(Dp(4.0)))
                } else {
                    Box(Modifier::new()
                        .size(Dp(14.0), Dp(14.0))
                        .on_click(move || {
                            s_close.ws.borrow_mut().close_tab(i);
                            s_close.bump();
                        }))
                    .child(
                        Icon(Symbols::close)
                            .size(Sp(13.0))
                            .color(th.on_surface_variant),
                    )
                },
            ))
        })
        .collect();

    Column(Modifier::new().fill_max_width()).child((
        Row(Modifier::new()
            .fill_max_width()
            .height(Dp(layout::TAB_H))
            .background(th.surface_container))
        .child((
            items,
            {
                let s = session.clone();
                Box(Modifier::new()
                    .size(Dp(layout::TAB_H), Dp(layout::TAB_H))
                    .on_click(move || {
                        use rext_core::Buffer;
                        s.ws.borrow_mut().open_buffer(Buffer::empty());
                        s.bump();
                    }))
                .child(
                    Icon(Symbols::add)
                        .size(Sp(16.0))
                        .color(th.on_surface_variant),
                )
            },
        )),
        Box(Modifier::new()
            .fill_max_width()
            .height(Dp(1.0))
            .background(th.outline_variant)),
    ))
}
