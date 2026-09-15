#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use repose_canvas::{Canvas, DrawScope};
use repose_core::prelude::*;
use repose_material::{Icon, material_symbols};
use repose_ui::*;

use crate::chrome::{layout::*, zed_chrome};
use crate::session::SessionRef;
use crate::term_grid::{DEFAULT_BG, DEFAULT_FG, PALETTE, SharedGrid, new_shared_grid};
use crate::term_pty::PtySession;

material_symbols! {
    terminal: '\u{EB8E}',
    add: '\u{E145}',
    close: '\u{E5CD}',
}

fn hex_color(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

struct TermTab {
    session: Option<PtySession>,
    grid: SharedGrid,
    scroll: f32,
    title: String,
    dead: bool,
}

fn spawn_tab(cwd: std::path::PathBuf, cols: u16, rows: u16) -> TermTab {
    let grid = new_shared_grid(cols as usize, rows as usize);
    match PtySession::spawn(grid.clone(), cwd, cols, rows, None) {
        Ok(s) => {
            let title = s.title.lock().map(|t| t.clone()).unwrap_or_default();
            TermTab {
                session: Some(s),
                grid,
                scroll: 0.0,
                title,
                dead: false,
            }
        }
        Err(e) => {
            log::error!("pty spawn: {e:?}");
            TermTab {
                session: None,
                grid,
                scroll: 0.0,
                title: format!("spawn failed: {e}"),
                dead: true,
            }
        }
    }
}

pub fn TerminalPanel(session: SessionRef) -> View {
    let chrome = zed_chrome();
    let th = theme();
    let cwd = session.ws.borrow().root.clone();

    let tabs: Rc<RefCell<Vec<TermTab>>> =
        remember_with_key("rext:term_tabs", || RefCell::new(Vec::new()));
    let active: Rc<RefCell<usize>> =
        remember_with_key("rext:term_active", || RefCell::new(0usize));

    if tabs.borrow().is_empty() {
        tabs.borrow_mut().push(spawn_tab(cwd.clone(), 80, 24));
    }

    {
        let mut ts = tabs.borrow_mut();
        let mut a = active.borrow_mut();
        if *a >= ts.len() {
            *a = ts.len().saturating_sub(1);
        }
        for t in ts.iter_mut() {
            if t.session.as_ref().is_some_and(|s| !s.is_alive()) && !t.dead {
                t.dead = true;
                t.title = format!("{} (exited)", t.title);
            }
        }
    }

    let tab_count = tabs.borrow().len();
    let ai = *active.borrow();

    let mut header_items: Vec<View> = Vec::with_capacity(tab_count + 2);
    for (i, t) in tabs.borrow().iter().enumerate() {
        let is_active = i == ai;
        let a2 = active.clone();
        let tabs_close = tabs.clone();
        let title = if t.title.len() > 18 {
            t.title.chars().take(18).collect::<String>()
        } else {
            t.title.clone()
        };
        header_items.push(
            Row(Modifier::new()
                .key(i as u64)
                .height(Dp(24.0))
                .padding(Dp(8.0))
                .gap(Dp(6.0))
                .align_items(AlignItems::CENTER)
                .background(if is_active {
                    th.surface_container_high
                } else {
                    Color::TRANSPARENT
                })
                .clip_rounded(Dp(4.0))
                .on_click(move || {
                    *a2.borrow_mut() = i;
                    repose_core::request_frame();
                }))
            .child((
                Text(title)
                    .size(Sp(11.0))
                    .color(if is_active {
                        th.on_surface
                    } else {
                        th.on_surface_variant
                    }),
                Box(Modifier::new()
                    .size(Dp(14.0), Dp(14.0))
                    .on_click(move || {
                        let mut ts = tabs_close.borrow_mut();
                        if let Some(t) = ts.get(i) {
                            if let Some(s) = &t.session {
                                s.kill();
                            }
                        }
                        if ts.len() > 1 {
                            ts.remove(i);
                        }
                        repose_core::request_frame();
                    }))
                .child(
                    Icon(Symbols::close)
                        .size(Sp(11.0))
                        .color(th.on_surface_variant),
                ),
            )),
        );
    }
    {
        let tabs_new = tabs.clone();
        let cwd_new = cwd.clone();
        header_items.push(
            Box(Modifier::new()
                .size(Dp(24.0), Dp(24.0))
                .on_click(move || {
                    tabs_new
                        .borrow_mut()
                        .push(spawn_tab(cwd_new.clone(), 80, 24));
                    repose_core::request_frame();
                }))
            .child(
                Icon(Symbols::add)
                    .size(Sp(14.0))
                    .color(th.on_surface_variant),
            ),
        );
    }

    let header = Row(Modifier::new()
        .fill_max_width()
        .height(Dp(28.0))
        .padding(Dp(6.0))
        .gap(Dp(4.0))
        .align_items(AlignItems::CENTER)
        .background(th.surface_container))
    .child((
        Icon(Symbols::terminal)
            .size(Sp(13.0))
            .color(th.on_surface_variant),
        header_items,
        Spacer(),
        Text(cwd.display().to_string())
            .size(Sp(10.0))
            .color(th.on_surface_variant),
    ));

    let body = {
        let t = tabs.borrow();
        let Some(tab) = t.get(ai) else {
            return Column(Modifier::new().fill_max_size()).child((header,));
        };
        if tab.session.is_none() {
            let msg = tab.title.clone();
            Column(Modifier::new()
                .fill_max_size()
                .background(hex_color(DEFAULT_BG))
                .padding(Dp(12.0)))
            .child(
                Text(format!("Terminal unavailable on this platform ({msg})"))
                    .size(Sp(12.0))
                    .color(hex_color(DEFAULT_FG)),
            )
        } else {
            drop(t);
            term_canvas(tabs.clone(), ai, chrome.text)
        }
    };

    Column(Modifier::new().fill_max_size()).child((header, body))
}

fn term_canvas(tabs: Rc<RefCell<Vec<TermTab>>>, ai: usize, _text: Color) -> View {
    let grid: SharedGrid = tabs.borrow().get(ai).map(|t| t.grid.clone()).unwrap();
    let tabs_key = tabs.clone();
    let tabs_scroll = tabs.clone();
    let tabs_input = tabs.clone();
    let tabs_resize = tabs.clone();

    Canvas(
        Modifier::new()
            .fill_max_size()
            .background(hex_color(DEFAULT_BG))
            .focusable(true)
            .on_scroll(move |delta: Vec2| {
                if let Some(t) = tabs_scroll.borrow_mut().get_mut(ai) {
                    t.scroll = (t.scroll + delta.y).max(0.0);
                }
                Vec2::ZERO
            })
            .on_key_event(move |ev: KeyEvent| {
                use repose_core::input::{Key, KeyEventType};
                if ev.event_type != KeyEventType::Down {
                    return false;
                }
                let session_alive = tabs_input
                    .borrow()
                    .get(ai)
                    .and_then(|tab| tab.session.as_ref().map(|_| ()))
                    .is_some();
                let m = ev.modifiers;
                let bytes: Vec<u8> = match ev.key {
                    Key::Enter => b"\r".to_vec(),
                    Key::Backspace => b"\x7f".to_vec(),
                    Key::Tab => b"\t".to_vec(),
                    Key::Escape => b"\x1b".to_vec(),
                    Key::ArrowUp => {
                        if m.alt {
                            return false;
                        }
                        b"\x1b[A".to_vec()
                    }
                    Key::ArrowDown => {
                        if m.alt {
                            return false;
                        }
                        b"\x1b[B".to_vec()
                    }
                    Key::ArrowRight => b"\x1b[C".to_vec(),
                    Key::ArrowLeft => b"\x1b[D".to_vec(),
                    Key::Home => b"\x1b[H".to_vec(),
                    Key::End => b"\x1b[F".to_vec(),
                    Key::PageUp => b"\x1b[5~".to_vec(),
                    Key::PageDown => b"\x1b[6~".to_vec(),
                    Key::Delete => b"\x1b[3~".to_vec(),
                    Key::Insert => b"\x1b[2~".to_vec(),
                    Key::Character(c) => {
                        if m.command || m.ctrl {
                            match c {
                                'c' | 'C' => b"\x03".to_vec(),
                                'd' | 'D' => b"\x04".to_vec(),
                                'z' | 'Z' => b"\x1a".to_vec(),
                                'l' | 'L' => b"\x0c".to_vec(),
                                'a' | 'A' => b"\x01".to_vec(),
                                'e' | 'E' => b"\x05".to_vec(),
                                'k' | 'K' => b"\x0b".to_vec(),
                                'u' | 'U' => b"\x15".to_vec(),
                                'w' | 'W' => b"\x17".to_vec(),
                                'r' | 'R' => b"\x12".to_vec(),
                                _ => return false,
                            }
                        } else if m.alt {
                            let mut v = vec![0x1b];
                            let mut buf = [0u8; 4];
                            v.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                            v
                        } else {
                            c.to_string().into_bytes()
                        }
                    }
                    Key::Space => b" ".to_vec(),
                    Key::F(n) => match n {
                        1 => b"\x1bOP".to_vec(),
                        2 => b"\x1bOQ".to_vec(),
                        3 => b"\x1bOR".to_vec(),
                        4 => b"\x1bOS".to_vec(),
                        _ => return false,
                    },
                    Key::Unknown => return false,
                };
                if !session_alive {
                    return false;
                }
                let tabs = tabs_input.clone();
                let t = tabs.borrow();
                let Some(tab) = t.get(ai) else {
                    return false;
                };
                let Some(s) = &tab.session else {
                    return false;
                };
                s.write_input(&bytes);
                drop(t);
                if let Some(t) = tabs.borrow_mut().get_mut(ai) {
                    t.scroll = 0.0;
                }
                true
            }),
        move |scope: &mut DrawScope| {
            let w = scope.size.width;
            let h = scope.size.height;
            let cols = ((w / (FONT_PX * 0.6)) as usize).max(20).min(400);
            let rows = ((h / LINE_H) as usize).max(5).min(200);
            if let Some(t) = tabs_key.borrow().get(ai) {
                if let Some(s) = &t.session {
                    let (cc, rr) = (cols as u16, rows as u16);
                    if let Ok(g) = t.grid.lock() {
                        if g.cols != cols || g.rows != rows {
                            drop(g);
                            s.resize(cc, rr);
                        }
                    }
                }
            }
            paint_term(scope, &grid, tabs_resize.clone(), ai, w, h, cols, rows);
        },
    )
}

fn paint_term(
    scope: &mut DrawScope,
    grid: &SharedGrid,
    tabs: Rc<RefCell<Vec<TermTab>>>,
    ai: usize,
    w: f32,
    h: f32,
    cols: usize,
    rows: usize,
) {
    let Ok(g) = grid.lock() else {
        return;
    };
    let scroll_lines = tabs
        .borrow()
        .get(ai)
        .map(|t| (t.scroll / LINE_H) as usize)
        .unwrap_or(0);
    let visible = g.visible();
    let total_extra = g.total_lines().saturating_sub(rows);
    let skip = scroll_lines.min(total_extra);
    let start = visible.len().saturating_sub(rows + skip);
    let baseline = (LINE_H - FONT_PX) * 0.5;
    let cell_w = FONT_PX * 0.6;

    let mut row_idx = 0;
    for (vi, line) in visible.iter().skip(start).take(rows).enumerate() {
        let y = row_idx as f32 * LINE_H;
        row_idx += 1;
        let mut x = TEXT_PAD;
        let mut ci = 0;
        while ci < cols.min(line.len()) {
            let cell = &line[ci];
            let cw = cell_w;
            let bg = match cell.attr.bg_kind {
                0 => None,
                1 => Some(PALETTE[(cell.attr.bg & 15) as usize]),
                _ => Some(cell.attr.bg),
            };
            if let Some(bg) = bg {
                scope.draw_rect(
                    Rect {
                        x,
                        y,
                        w: cw,
                        h: LINE_H,
                    },
                    hex_color(bg),
                    Px(0.0),
                );
            }
            if cell.ch != ' ' {
                let fg = match cell.attr.fg_kind {
                    0 => DEFAULT_FG,
                    1 => PALETTE[(cell.attr.fg & 15) as usize],
                    _ => cell.attr.fg,
                };
                scope.draw_text(
                    cell.ch.to_string(),
                    Vec2 { x, y: y + baseline },
                    hex_color(fg),
                    Px(FONT_PX),
                );
            }
            x += cw;
            ci += 1;
            let _ = vi;
        }
    }

    if scroll_lines == 0 && g.cursor_visible {
        let (cr, cc) = g.cursor;
        if cr < rows {
            let y = cr as f32 * LINE_H;
            let x = TEXT_PAD + cc as f32 * cell_w;
            scope.draw_rect(
                Rect {
                    x,
                    y: y + 2.0,
                    w: (cell_w * 0.9).max(4.0),
                    h: LINE_H - 4.0,
                },
                hex_color(DEFAULT_FG),
                Px(0.0),
            );
        }
    }

    let _ = (w, h);
}
