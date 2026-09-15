#![allow(non_snake_case)]
use repose_core::prelude::*;
use repose_ui::*;
use crate::session::SessionRef;

pub fn StatusBar(session: SessionRef, height: f32) -> View {
    let th = theme();
    let (title, dirty, msg, pos, lines, sel_len, can_undo, can_redo) = {
        let ws = session.ws.borrow();
        let h = ws.active();
        (
            h.map(|x| x.buffer.title()).unwrap_or_default(),
            h.map(|x| x.buffer.dirty()).unwrap_or(false),
            ws.status.clone(),
            h.map(|x| x.head()).unwrap_or_default(),
            h.map(|x| x.buffer.len_lines()).unwrap_or(0),
            h.map(|x| {
                x.selections
                    .ranges(&x.buffer)
                    .into_iter()
                    .map(|(a, b)| b - a)
                    .sum()
            })
            .unwrap_or(0),
            h.map(|x| x.history.can_undo()).unwrap_or(false),
            h.map(|x| x.history.can_redo()).unwrap_or(false),
        )
    };

    Row(Modifier::new()
        .fill_max_width()
        .height(Dp(height))
        .background(th.surface_container)
        .padding(Dp(10.0))
        .align_items(AlignItems::CENTER)
        .gap(Dp(14.0)))
    .child((
        Text(format!("{}{}", title, if dirty { " ●" } else { "" }))
            .size(Sp(11.0))
            .color(th.on_surface),
        Text(msg).size(Sp(11.0)).color(th.on_surface_variant),
        Spacer(),
        if sel_len > 0 {
            Text(format!("{sel_len} selected"))
                .size(Sp(11.0))
                .color(th.primary)
        } else {
            Text(format!("{lines} lines"))
                .size(Sp(11.0))
                .color(th.on_surface_variant)
        },
        Text(format!("Ln {}, Col {}", pos.row + 1, pos.column + 1))
            .size(Sp(11.0))
            .color(th.on_surface),
        Text(if can_undo { "↩" } else { " " }.to_string())
            .size(Sp(11.0))
            .color(if can_undo {
                th.on_surface_variant
            } else {
                th.surface_container
            }),
        Text(if can_redo { "↪" } else { " " }.to_string())
            .size(Sp(11.0))
            .color(if can_redo {
                th.on_surface_variant
            } else {
                th.surface_container
            }),
        Text("UTF-8".to_string())
            .size(Sp(11.0))
            .color(th.on_surface_variant),
    ))
}
