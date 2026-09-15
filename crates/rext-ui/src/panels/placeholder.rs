#![allow(non_snake_case)]
use repose_core::*;
use repose_ui::*;

pub fn Placeholder(msg: &'static str) -> View {
    let th = theme();
    Box(Modifier::new()
        .fill_max_size()
        .background(th.surface_container)
        .padding(Dp(12.0)))
    .child(
        Text(msg.to_string())
            .size(Sp(12.0))
            .color(th.on_surface_variant),
    )
}
