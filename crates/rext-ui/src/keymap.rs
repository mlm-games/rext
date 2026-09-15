use rext_core::Cmd;
use repose_core::input::{Key, KeyEvent, KeyEventType};

pub struct Action {
    pub cmd: Cmd,
    pub extend: bool,
    /// Handled as app chrome, not buffer.
    pub app: Option<AppAction>,
}

#[derive(Clone, Copy, Debug)]
pub enum AppAction {
    TogglePalette,
    ClosePalette,
    ToggleFind,
    FindNext,
    FindPrev,
}

pub fn translate(ev: &KeyEvent, page_rows: usize) -> Option<Action> {
    if ev.event_type != KeyEventType::Down {
        return None;
    }
    let m = ev.modifiers;
    let chord = m.command || m.ctrl;
    let shift = m.shift;

    if chord {
        match ev.key {
            Key::Character('p') | Key::Character('P') => {
                return Some(Action {
                    cmd: Cmd::Save, // placeholder unused
                    extend: false,
                    app: Some(AppAction::TogglePalette),
                });
            }
            Key::Character('s') | Key::Character('S') => {
                return Some(Action {
                    cmd: Cmd::Save,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('z') | Key::Character('Z') if shift => {
                return Some(Action {
                    cmd: Cmd::Redo,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('z') | Key::Character('Z') => {
                return Some(Action {
                    cmd: Cmd::Undo,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('y') | Key::Character('Y') => {
                return Some(Action {
                    cmd: Cmd::Redo,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('a') | Key::Character('A') => {
                return Some(Action {
                    cmd: Cmd::SelectAll,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('c') | Key::Character('C') => {
                return Some(Action {
                    cmd: Cmd::Copy,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('x') | Key::Character('X') => {
                return Some(Action {
                    cmd: Cmd::Cut,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('v') | Key::Character('V') => {
                return Some(Action {
                    cmd: Cmd::Paste,
                    extend: false,
                    app: None,
                });
            }
            Key::Character('f') | Key::Character('F') => {
                return Some(Action {
                    cmd: Cmd::Save,
                    extend: false,
                    app: Some(AppAction::ToggleFind),
                });
            }
            _ => {}
        }
    }

    if matches!(ev.key, Key::Escape) {
        return Some(Action {
            cmd: Cmd::Save,
            extend: false,
            app: Some(AppAction::ClosePalette),
        });
    }

    // F3 / Shift+F3: find next/prev (also when find bar focused — the bar
    // doesn't consume them, so they bubble here).
    match ev.key {
        Key::F(3) if shift => {
            return Some(Action {
                cmd: Cmd::Save,
                extend: false,
                app: Some(AppAction::FindPrev),
            });
        }
        Key::F(3) => {
            return Some(Action {
                cmd: Cmd::Save,
                extend: false,
                app: Some(AppAction::FindNext),
            });
        }
        _ => {}
    }

    // Ctrl+Arrows / Ctrl+Backspace/Delete: word-wise ops (no chord needed
    // for plain arrows; word variants require the chord).
    if chord {
        match ev.key {
            Key::ArrowLeft => {
                return Some(Action {
                    cmd: Cmd::MoveWordLeft,
                    extend: shift,
                    app: None,
                });
            }
            Key::ArrowRight => {
                return Some(Action {
                    cmd: Cmd::MoveWordRight,
                    extend: shift,
                    app: None,
                });
            }
            Key::Backspace => {
                return Some(Action {
                    cmd: Cmd::DeleteWordLeft,
                    extend: false,
                    app: None,
                });
            }
            Key::Delete => {
                return Some(Action {
                    cmd: Cmd::DeleteWordRight,
                    extend: false,
                    app: None,
                });
            }
            _ => {}
        }
    }

    let cmd = match ev.key {
        Key::ArrowLeft => Cmd::MoveLeft,
        Key::ArrowRight => Cmd::MoveRight,
        Key::ArrowUp => Cmd::MoveUp,
        Key::ArrowDown => Cmd::MoveDown,
        Key::Home if chord => Cmd::MoveDocStart,
        Key::End if chord => Cmd::MoveDocEnd,
        Key::Home => Cmd::MoveHome,
        Key::End => Cmd::MoveEnd,
        Key::PageUp => Cmd::MovePageUp(page_rows),
        Key::PageDown => Cmd::MovePageDown(page_rows),
        Key::Backspace => Cmd::Backspace,
        Key::Delete => Cmd::Delete,
        Key::Enter => Cmd::Newline,
        Key::Tab => Cmd::Tab,
        Key::Character(c) if !chord && !m.alt && !c.is_control() => Cmd::Insert(c.to_string()),
        Key::Space if !chord => Cmd::Insert(" ".into()),
        _ => return None,
    };

    Some(Action {
        cmd,
        extend: shift,
        app: None,
    })
}
