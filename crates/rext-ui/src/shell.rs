use std::cell::RefCell;
use std::rc::Rc;

use repose_core::prelude::*;
use repose_docking::*;
use repose_ui::*;

use crate::palette::CommandPalette;
use crate::panels;
use crate::session::SessionRef;
use crate::chrome::layout;

pub const PANEL_FILES: PanelId = 1;
pub const PANEL_EDITOR: PanelId = 2;
pub const PANEL_PROBLEMS: PanelId = 3;
pub const PANEL_TERMINAL: PanelId = 4;

fn default_dock() -> DockState {
    let root = DockNode {
        id: 1,
        kind: DockKind::Split {
            dir: SplitDir::Horizontal,
            ratio: 0.20,
            a: Box::new(DockNode {
                id: 2,
                kind: DockKind::Tabs {
                    tabs: vec![PANEL_FILES],
                    active: Some(PANEL_FILES),
                },
            }),
            b: Box::new(DockNode {
                id: 3,
                kind: DockKind::Split {
                    dir: SplitDir::Vertical,
                    ratio: 0.78,
                    a: Box::new(DockNode {
                        id: 4,
                        kind: DockKind::Tabs {
                            tabs: vec![PANEL_EDITOR],
                            active: Some(PANEL_EDITOR),
                        },
                    }),
                    b: Box::new(DockNode {
                        id: 5,
                        kind: DockKind::Tabs {
                            tabs: vec![PANEL_PROBLEMS, PANEL_TERMINAL],
                            active: Some(PANEL_TERMINAL),
                        },
                    }),
                },
            }),
        },
    };
    DockState::from_root(root, 99)
}

pub fn EditorShell(session: SessionRef) -> View {
    let th = theme();
    // showcase pattern: remember_with_key → Rc<RefCell<DockState>>
    let dock = remember_with_key("rext:dock", || RefCell::new(default_dock()));

    let s_files = session.clone();
    let s_ed = session.clone();
    let panels = vec![
        DockPanel {
            id: PANEL_FILES,
            title: "Project".into(),
            content: Rc::new(move || panels::file_tree::FileTreePanel(s_files.clone())),
        },
        DockPanel {
            id: PANEL_EDITOR,
            title: "Editor".into(),
            content: Rc::new(move || panels::editor::EditorPanel(s_ed.clone())),
        },
        DockPanel {
            id: PANEL_PROBLEMS,
            title: "Problems".into(),
            content: Rc::new(|| panels::placeholder::Placeholder("Diagnostics (LSP phase 4)")),
        },
        DockPanel {
            id: PANEL_TERMINAL,
            title: "Terminal".into(),
            content: Rc::new(|| panels::placeholder::Placeholder("Terminal (portable-pty later)")),
        },
    ];

    ZStack(Modifier::new().fill_max_size()).child((
        Column(Modifier::new().fill_max_size()).child((
            DockArea(
                "rext_dock",
                Modifier::new()
                    .fill_max_width()
                    .flex_grow(1.0)
                    .background(th.background),
                dock,
                panels,
                DockCallbacks::default(),
            ),
            Box(Modifier::new()
                .fill_max_width()
                .height(Dp(1.0))
                .background(th.outline_variant)),
            panels::status_bar::StatusBar(session.clone(), layout::STATUS_H),
        )),
        CommandPalette(session),
    ))
}
