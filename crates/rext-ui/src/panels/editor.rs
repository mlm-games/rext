#![allow(non_snake_case)]
use repose_core::prelude::*;
use repose_ui::*;
use super::{editor_surface::EditorSurface, tab_bar::TabBar};
use crate::session::SessionRef;

pub fn EditorPanel(session: SessionRef) -> View {
    let th = theme();
    Column(
        Modifier::new()
            .fill_max_size()
            .background(th.surface_container_lowest),
    )
    .child((
        TabBar(session.clone()),
        Box(Modifier::new().fill_max_width().flex_grow(1.0)).child(EditorSurface(session)),
    ))
}
