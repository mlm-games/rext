#![allow(non_snake_case)]

mod chrome;
mod keymap;
mod metrics_cache;
mod palette;
mod panels;
mod session;
mod shell;
mod term_grid;
mod term_pty;
mod term_vte;

use repose_core::*;
use repose_ui::*;

use session::use_session;
use shell::EditorShell;

/// Same entry shape as renamite-ui / showcase.
pub fn app(_s: &mut Scheduler, _rc: &RenderContext) -> View {
    let session = use_session();
    // Subscribe: any bump() invalidates this composition.
    let _rev = session.rev.with(|r| *r);

    let th = theme();
    Box(Modifier::new().fill_max_size().background(th.background))
        .child(EditorShell(session))
}
