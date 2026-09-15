#![allow(non_snake_case)]
use repose_core::prelude::*;
use repose_ui::*;
use crate::session::SessionRef;

pub fn StatusBar(session: SessionRef, height: f32) -> View {
    let th = theme();
    let (title, dirty, msg, pos, lines) = {
        let ws = session.ws.borrow();
        let h = ws.active();
        (
            h.map(|x| x.buffer.title()).unwrap_or_default(),
            h.map(|x| x.buffer.dirty()).unwrap_or(false),
            ws.status.clone(),
            h.map(|x| x.selection.head).unwrap_or_default(),
            h.map(|x| x.buffer.len_lines()).unwrap_or(0),
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
        Text(format!("{lines} lines"))
            .size(Sp(11.0))
            .color(th.on_surface_variant),
        Text(format!("Ln {}, Col {}", pos.row + 1, pos.column + 1))
            .size(Sp(11.0))
            .color(th.on_surface),
        Text("UTF-8".to_string())
            .size(Sp(11.0))
            .color(th.on_surface_variant),
    ))
}
