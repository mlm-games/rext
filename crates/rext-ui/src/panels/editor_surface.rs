#![allow(non_snake_case)]
//! The text surface. Same shape as renamite-ui/src/panels/viewport.rs -
//! a `repose_canvas::Canvas` with pointer/key/scroll modifiers, painting only
//! the visible line window. Coordinates are px (see repose-canvas docs).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use rext_core::{Cmd, DisplayPoint, Point};
use repose_canvas::{Canvas, DrawScope};
use repose_core::prelude::*;
use repose_ui::*;

use crate::chrome::{layout::*, zed_chrome};
use crate::keymap::{self, AppAction};
use crate::metrics_cache::{CODE_FAMILY, LineMetricsCache};
use crate::session::SessionRef;

pub fn EditorSurface(session: SessionRef) -> View {
    let chrome = zed_chrome();
    // Cell is Copy-friendly; store it inside a Mutable-friendly Rc via remember.
    let size_px: Rc<Cell<(f32, f32)>> =
        remember_with_key("rext:ed_size", || Cell::new((800.0f32, 600.0f32)));
    let dragging: Rc<Cell<bool>> =
        remember_with_key("rext:ed_drag", || Cell::new(false));
    let metrics: Rc<RefCell<LineMetricsCache>> =
        remember_with_key("rext:ed_metrics", || RefCell::new(LineMetricsCache::default()));

    let gutter_w = {
        let ws = session.ws.borrow();
        let n = ws
            .active()
            .map(|h| h.buffer.len_lines())
            .unwrap_or(1)
            .max(1);
        let digits = ((n as f32).log10().floor() as usize) + 1;
        GUTTER_PAD * 2.0 + digits.max(2) as f32 * (FONT_PX * 0.6)
    };

    let page_rows = {
        let h = size_px.get().1;
        ((h / LINE_H).floor() as usize).max(1)
    };

    let s_key = session.clone();
    let s_down = session.clone();
    let s_move = session.clone();
    let s_scroll = session.clone();
    let s_paint = session.clone();
    let size_paint = size_px.clone();
    let drag_flag = dragging;
    let drag_move = drag_flag.clone();
    let drag_up = drag_flag.clone();
    let metrics_paint = metrics.clone();
    let metrics_hit = metrics;

    let hit = Rc::new(move |x: f32, y: f32| -> Point {
        let ws = session.ws.borrow();
        let Some(h) = ws.active() else {
            return Point::default();
        };
        let row = h.scroll_row + (y / LINE_H).floor().max(0.0) as usize;
        let row = row.min(h.buffer.len_lines().saturating_sub(1));
        let display = h.display.display_line(&h.buffer, row);
        let mut cache = metrics_hit.borrow_mut();
        let m = cache.metrics(h.buffer.version(), row, &display, FONT_PX);
        let local_x = (x - gutter_w - TEXT_PAD).max(0.0);
        let dcol = LineMetricsCache::col_for_x(m, local_x);
        h.display.display_to_point(
            &h.buffer,
            DisplayPoint {
                row,
                column: dcol,
            },
        )
    });
    let hit_down = hit.clone();
    let hit_move = hit;

    Canvas(
        Modifier::new()
            .fill_max_size()
            .background(chrome.editor_bg)
            .focusable(true)
            .on_pointer_down(move |ev: PointerEvent| {
                if !matches!(ev.event, PointerEventKind::Down(PointerButton::Primary)) {
                    return;
                }
                drag_flag.set(true);
                let p = hit_down(ev.position.x, ev.position.y);
                if ev.modifiers.alt {
                    s_down.add_cursor(p);
                } else {
                    s_down.dispatch(
                        Cmd::SetCaret {
                            point: p,
                            extend: ev.modifiers.shift,
                        },
                        false,
                    );
                }
                ev.consume();
            })
            .on_pointer_move(move |ev: PointerEvent| {
                if !drag_move.get() {
                    return;
                }
                let p = hit_move(ev.position.x, ev.position.y);
                s_move.dispatch(
                    Cmd::SetCaret {
                        point: p,
                        extend: true,
                    },
                    false,
                );
            })
            .on_pointer_up(move |_ev: PointerEvent| {
                drag_up.set(false);
            })
            .on_scroll(move |delta: Vec2| {
                // delta.y > 0 typically means scroll content up / view down
                let rows = (delta.y / LINE_H).round() as isize;
                if rows != 0 {
                    s_scroll.dispatch(Cmd::ScrollBy { rows }, false);
                }
                Vec2::ZERO // consumed
            })
            .on_key_event(move |ev: KeyEvent| {
                let Some(act) = keymap::translate(&ev, page_rows) else {
                    return false;
                };
                match act.app {
                    Some(AppAction::TogglePalette) => {
                        s_key.palette_open.update(|v| *v = !*v);
                        true
                    }
                    Some(AppAction::ClosePalette) => {
                        if *s_key.palette_open.get() {
                            s_key.palette_open.set(false);
                            true
                        } else if *s_key.find_open.get() {
                            s_key.find_open.set(false);
                            true
                        } else {
                            false
                        }
                    }
                    Some(AppAction::ToggleFind) => {
                        let was_open = *s_key.find_open.get();
                        if !was_open {
                            // Pre-fill from primary selection.
                            let ws = s_key.ws.borrow();
                            if let Some(h) = ws.active() {
                                let (a, b) = h.selections.primary().range_offsets(&h.buffer);
                                if a != b {
                                    let text = h.buffer.slice_chars(a, b);
                                    if !text.contains('\n') && text.chars().count() <= 80 {
                                        s_key.find_query.set(text);
                                    }
                                }
                            }
                            s_key.find_match_idx.set(0);
                        }
                        s_key.find_open.set(!was_open);
                        true
                    }
                    Some(AppAction::FindNext) => {
                        s_key.find_match_idx.update(|v| {
                            let (n, _) = crate::panels::find_bar::find_matches(&s_key);
                            *v = if n == 0 { 0 } else { (*v + 1) % n };
                        });
                        jump_to_find(&s_key);
                        true
                    }
                    Some(AppAction::FindPrev) => {
                        s_key.find_match_idx.update(|v| {
                            let (n, _) = crate::panels::find_bar::find_matches(&s_key);
                            *v = if n == 0 { 0 } else { (*v + n - 1) % n };
                        });
                        jump_to_find(&s_key);
                        true
                    }
                    None => {
                        s_key.dispatch(act.cmd, act.extend);
                        true
                    }
                }
            }),
        move |scope: &mut DrawScope| {
            paint(
                scope,
                &s_paint,
                &chrome,
                gutter_w,
                &size_paint,
                &mut metrics_paint.borrow_mut(),
            );
        },
    )
}

fn jump_to_find(session: &SessionRef) {
    let (_, matches) = crate::panels::find_bar::find_matches(session);
    if matches.is_empty() {
        return;
    }
    let idx = (*session.find_match_idx.get()).min(matches.len() - 1);
    let (row, sc, _) = matches[idx];
    session.jump_to_active(rext_core::Point { row, column: sc });
}

fn paint(
    scope: &mut DrawScope,
    session: &SessionRef,
    chrome: &crate::chrome::EditorChrome,
    gutter_w: f32,
    size_px: &Cell<(f32, f32)>,
    cache: &mut LineMetricsCache,
) {
    let w = scope.size.width;
    let h = scope.size.height;
    size_px.set((w, h));

    let ws = session.ws.borrow();
    let Some(ed) = ws.active() else {
        // No buffer (shouldn't normally happen — Workspace keeps a scratch
        // buffer — but paint the empty state instead of a dead canvas).
        scope.draw_rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            },
            chrome.editor_bg,
            Px(0.0),
        );
        scope.draw_text(
            "No open buffer — Ctrl+P → File: New Untitled",
            Vec2 {
                x: gutter_w + TEXT_PAD,
                y: h * 0.4,
            },
            chrome.gutter_fg_active,
            Px(FONT_PX),
        );
        return;
    };
    let buf = &ed.buffer;
    let first = ed.scroll_row;
    let visible = (h / LINE_H).ceil() as usize + 2;
    let last = (first + visible).min(buf.len_lines());
    let text_x = gutter_w + TEXT_PAD;
    let baseline = (LINE_H - FONT_PX) * 0.5;

    scope.draw_rect(
        Rect {
            x: 0.0,
            y: 0.0,
            w: gutter_w,
            h,
        },
        chrome.gutter_bg,
        Px(0.0),
    );

    let sel_ranges = ed.selections.ranges(buf);
    let primary = ed.selections.primary();
    let caret = primary.head();
    let caret_dp = ed.display.point_to_display(buf, caret);
    let caret_rows: Vec<usize> = ed
        .selections
        .all()
        .iter()
        .map(|s| s.head().row)
        .collect();

    let (find_matches, find_active) = if *session.find_open.get() {
        let (_, m) = crate::panels::find_bar::find_matches(session);
        let a = (*session.find_match_idx.get()).min(m.len().saturating_sub(1));
        (m, a)
    } else {
        (Vec::new(), 0)
    };

    // Diff hunks for the visible window, resolved to (row_range, color).
    // Deleted hunks are zero-width at the deletion row: paint the marker on
    // that row itself. `session.diffs` is a separate RefCell from `ws`, so
    // borrowing both is safe.
    let diff_marks: Vec<(std::ops::Range<usize>, Color)> = {
        let active_id = ws.active_id();
        let diffs = session.diffs.borrow();
        match active_id.and_then(|id| diffs.get(&id)) {
            None => Vec::new(),
            Some(d) => d
                .hunks_intersecting_rows(first..last)
                .map(|h| {
                    let color = match h.kind {
                        rext_core::HunkKind::Added => chrome.diff_added,
                        rext_core::HunkKind::Modified => chrome.diff_modified,
                        rext_core::HunkKind::Deleted => chrome.diff_deleted,
                    };
                    (h.buffer_rows.clone(), color)
                })
                .collect(),
        }
    };

    for row in first..last {
        let y = (row - first) as f32 * LINE_H;
        let display = ed.display.display_line(buf, row);

        if row == caret.row {
            scope.draw_rect(
                Rect {
                    x: 0.0,
                    y,
                    w,
                    h: LINE_H,
                },
                chrome.current_line,
                Px(0.0),
            );
        }

        // find matches (all subtle, active one stronger)
        if !find_matches.is_empty() {
            let m = cache.metrics(buf.version(), row, &display, FONT_PX);
            for (i, (mr, mc0, mc1)) in find_matches.iter().enumerate() {
                if *mr != row {
                    continue;
                }
                let d0 = ed
                    .display
                    .point_to_display(buf, Point { row, column: *mc0 })
                    .column;
                let d1 = ed
                    .display
                    .point_to_display(buf, Point { row, column: *mc1 })
                    .column;
                let x0 = text_x + LineMetricsCache::x_for_col(m, d0);
                let x1 = text_x + LineMetricsCache::x_for_col(m, d1);
                let is_active = i == find_active;
                scope.draw_rect(
                    Rect {
                        x: x0,
                        y: y + LINE_H - 3.0,
                        w: (x1 - x0).max(2.0),
                        h: if is_active { 3.0 } else { 1.5 },
                    },
                    if is_active {
                        chrome.caret
                    } else {
                        chrome.gutter_fg
                    },
                    Px(0.0),
                );
            }
        }

        // selections (all cursors)
        for (sel_a, sel_b) in &sel_ranges {
            if sel_a == sel_b {
                continue;
            }
            let ls = buf.offset_of(Point { row, column: 0 });
            let le = ls + buf.line_len(row);
            let s = (*sel_a).max(ls);
            let en = (*sel_b).min(le);
            if s < en {
                let m = cache.metrics(buf.version(), row, &display, FONT_PX);
                let col0 = buf.point_of(s).column;
                let col1 = buf.point_of(en).column;
                let d0 = ed
                    .display
                    .point_to_display(buf, Point { row, column: col0 })
                    .column;
                let d1 = ed
                    .display
                    .point_to_display(buf, Point { row, column: col1 })
                    .column;
                let x0 = text_x + LineMetricsCache::x_for_col(m, d0);
                let x1 = text_x + LineMetricsCache::x_for_col(m, d1);
                scope.draw_rect(
                    Rect {
                        x: x0,
                        y,
                        w: (x1 - x0).max(2.0),
                        h: LINE_H,
                    },
                    chrome.selection,
                    Px(0.0),
                );
            }
        }

        // line number (UI font — gutter is chrome, not code)
        let num = format!("{}", row + 1);
        let nx = gutter_w - GUTTER_PAD - num.len() as f32 * FONT_PX * 0.55;
        scope.draw_text(
            num,
            Vec2 {
                x: nx,
                y: y + baseline,
            },
            if caret_rows.contains(&row) {
                chrome.gutter_fg_active
            } else {
                chrome.gutter_fg
            },
            Px(FONT_PX * 0.85),
        );

        for (range, color) in &diff_marks {
            // Non-empty ranges use contains; deleted (empty) hunks paint on
            // their anchor row.
            let hit = if range.is_empty() {
                range.start == row
            } else {
                range.contains(&row)
            };
            if hit {
                scope.draw_rect(
                    Rect {
                        x: gutter_w - 4.0,
                        y: y + 3.0,
                        w: 3.0,
                        h: LINE_H - 6.0,
                    },
                    *color,
                    Px(0.0),
                );
                break;
            }
        }

        if !display.is_empty() {
            // Code text: JetBrains Mono via font-awl `monospace` feature.
            // Same family as the metrics cache so caret/selection agree
            // with the painted glyphs.
            scope.draw_text_with_family(
                display,
                Vec2 {
                    x: text_x,
                    y: y + baseline,
                },
                chrome.text,
                Px(FONT_PX),
                CODE_FAMILY,
            );
        }
    }

    // carets (all cursors; primary last so it paints on top)
    let mut carets: Vec<_> = ed
        .selections
        .all()
        .iter()
        .map(|s| ed.display.point_to_display(buf, s.head()))
        .collect();
    carets.sort_by_key(|dp| {
        if dp.row == caret_dp.row && dp.column == caret_dp.column {
            1
        } else {
            0
        }
    });
    for cdp in carets {
        if cdp.row < first || cdp.row >= last {
            continue;
        }
        let y = (cdp.row - first) as f32 * LINE_H;
        let display = ed.display.display_line(buf, cdp.row);
        let m = cache.metrics(buf.version(), cdp.row, &display, FONT_PX);
        let cx = text_x + LineMetricsCache::x_for_col(m, cdp.column);
        scope.draw_rect(
            Rect {
                x: cx,
                y: y + 2.0,
                w: 2.0,
                h: LINE_H - 4.0,
            },
            chrome.caret,
            Px(0.0),
        );
    }

    scope.draw_rect(
        Rect {
            x: gutter_w,
            y: 0.0,
            w: 1.0,
            h,
        },
        chrome.border,
        Px(0.0),
    );
}
