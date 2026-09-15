use crate::buffer::{Buffer, BufferId, Point};
use crate::display_map::DisplayMap;
use crate::history::{Edit, History};
use crate::selection::Selection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct BufferHandle {
    pub buffer: Buffer,
    pub selection: Selection,
    pub history: History,
    pub scroll_row: usize,
    pub goal_column: Option<usize>,
    pub display: DisplayMap,
}

impl BufferHandle {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            selection: Selection::default(),
            history: History::default(),
            scroll_row: 0,
            goal_column: None,
            display: DisplayMap::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Cmd {
    Insert(String),
    Newline,
    Tab,
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveHome,
    MoveEnd,
    MoveDocStart,
    MoveDocEnd,
    MovePageUp(usize),
    MovePageDown(usize),
    SelectAll,
    Undo,
    Redo,
    Save,
    /// Click / programmatic caret.
    SetCaret { point: Point, extend: bool },
    ScrollBy { rows: isize },
}

pub struct Workspace {
    pub buffers: HashMap<BufferId, BufferHandle>,
    pub tab_order: Vec<BufferId>,
    pub active: usize,
    pub root: PathBuf,
    pub status: String,
}

impl Workspace {
    pub fn new(root: PathBuf, initial: Option<PathBuf>) -> Self {
        let mut ws = Self {
            buffers: HashMap::new(),
            tab_order: vec![],
            active: 0,
            root,
            status: "ready".into(),
        };
        if let Some(p) = initial {
            let _ = ws.open_path(&p);
        }
        if ws.tab_order.is_empty() {
            ws.open_buffer(Buffer::from_str(WELCOME));
        }
        ws
    }

    pub fn active_id(&self) -> Option<BufferId> {
        self.tab_order.get(self.active).copied()
    }

    pub fn active_mut(&mut self) -> Option<&mut BufferHandle> {
        let id = self.active_id()?;
        self.buffers.get_mut(&id)
    }

    pub fn active(&self) -> Option<&BufferHandle> {
        let id = self.active_id()?;
        self.buffers.get(&id)
    }

    pub fn open_buffer(&mut self, buffer: Buffer) -> BufferId {
        let id = buffer.id;
        self.buffers.insert(id, BufferHandle::new(buffer));
        self.tab_order.push(id);
        self.active = self.tab_order.len() - 1;
        id
    }

    pub fn open_path(&mut self, path: &Path) -> anyhow::Result<BufferId> {
        if let Some((i, id)) = self
            .tab_order
            .iter()
            .enumerate()
            .find(|(_, id)| self.buffers.get(id).and_then(|h| h.buffer.path()) == Some(path))
        {
            self.active = i;
            return Ok(*id);
        }
        let b = Buffer::open(path)?;
        Ok(self.open_buffer(b))
    }

    pub fn close_tab(&mut self, idx: usize) {
        if self.tab_order.len() <= 1 {
            return;
        }
        if idx >= self.tab_order.len() {
            return;
        }
        let id = self.tab_order.remove(idx);
        self.buffers.remove(&id);
        if self.active >= self.tab_order.len() {
            self.active = self.tab_order.len().saturating_sub(1);
        } else if idx < self.active {
            self.active -= 1;
        }
    }

    pub fn select_tab(&mut self, idx: usize) {
        if idx < self.tab_order.len() {
            self.active = idx;
        }
    }

    /// `extend` = shift for movement commands.
    pub fn exec(&mut self, cmd: Cmd, extend: bool) -> bool {
        match cmd {
            Cmd::Save => {
                if let Some(h) = self.active_mut() {
                    match h.buffer.save() {
                        Ok(()) => self.status = "saved".into(),
                        Err(e) => self.status = format!("save: {e}"),
                    }
                    return true;
                }
                false
            }
            Cmd::ScrollBy { rows } => {
                if let Some(h) = self.active_mut() {
                    let max = h.buffer.len_lines().saturating_sub(1);
                    h.scroll_row = (h.scroll_row as isize + rows).clamp(0, max as isize) as usize;
                    return true;
                }
                false
            }
            Cmd::SetCaret { point, extend } => {
                if let Some(h) = self.active_mut() {
                    h.history.seal();
                    if extend {
                        if h.selection.is_empty() {
                            h.selection.anchor = h.selection.head;
                        }
                        h.selection.head = clamp_point(&h.buffer, point);
                    } else {
                        h.selection = Selection::caret(clamp_point(&h.buffer, point));
                    }
                    h.goal_column = None;
                    ensure_visible(h, 40);
                    return true;
                }
                false
            }
            other => {
                let Some(h) = self.active_mut() else {
                    return false;
                };
                exec_on_handle(h, other, extend)
            }
        }
    }
}

fn exec_on_handle(h: &mut BufferHandle, cmd: Cmd, extend: bool) -> bool {
    use Cmd::*;
    match cmd {
        Insert(s) => replace_sel(h, &s, "type"),
        Newline => replace_sel(h, "\n", "newline"),
        Tab => replace_sel(h, "    ", "type"),
        Backspace => {
            if !h.selection.is_empty() {
                replace_sel(h, "", "delete");
            } else {
                let off = h.buffer.offset_of(h.selection.head);
                if off == 0 {
                    return false;
                }
                h.history.begin("backspace", h.selection.head);
                let text = h.buffer.slice_chars(off - 1, off);
                h.history.apply(
                    &mut h.buffer,
                    Edit::Delete {
                        at: off - 1,
                        text,
                    },
                );
                h.selection = Selection::caret(h.buffer.point_of(off - 1));
                h.history.commit(h.selection.head);
                h.goal_column = None;
            }
        }
        Delete => {
            if !h.selection.is_empty() {
                replace_sel(h, "", "delete");
            } else {
                let off = h.buffer.offset_of(h.selection.head);
                if off >= h.buffer.len_chars() {
                    return false;
                }
                h.history.begin("delete", h.selection.head);
                let text = h.buffer.slice_chars(off, off + 1);
                h.history
                    .apply(&mut h.buffer, Edit::Delete { at: off, text });
                h.history.commit(h.selection.head);
                h.goal_column = None;
            }
        }
        MoveLeft | MoveRight | MoveUp | MoveDown | MoveHome | MoveEnd | MoveDocStart
        | MoveDocEnd | MovePageUp(_) | MovePageDown(_) => {
            h.history.seal();
            if !extend {
                h.selection.anchor = h.selection.head;
            }
            match cmd {
                MoveLeft => {
                    let o = h.buffer.offset_of(h.selection.head);
                    h.selection.head = h.buffer.point_of(o.saturating_sub(1));
                    h.goal_column = None;
                }
                MoveRight => {
                    let o = h.buffer.offset_of(h.selection.head);
                    h.selection.head =
                        h.buffer.point_of((o + 1).min(h.buffer.len_chars()));
                    h.goal_column = None;
                }
                MoveUp => move_vert(h, -1),
                MoveDown => move_vert(h, 1),
                MovePageUp(n) => move_vert(h, -(n as isize)),
                MovePageDown(n) => move_vert(h, n as isize),
                MoveHome => {
                    h.selection.head.column = 0;
                    h.goal_column = None;
                }
                MoveEnd => {
                    h.selection.head.column = h.buffer.line_len(h.selection.head.row);
                    h.goal_column = None;
                }
                MoveDocStart => {
                    h.selection.head = Point::default();
                    h.goal_column = None;
                }
                MoveDocEnd => {
                    h.selection.head = h.buffer.point_of(h.buffer.len_chars());
                    h.goal_column = None;
                }
                _ => {}
            }
            if !extend {
                h.selection.anchor = h.selection.head;
            }
        }
        SelectAll => {
            h.history.seal();
            h.selection.anchor = Point::default();
            h.selection.head = h.buffer.point_of(h.buffer.len_chars());
        }
        Undo => {
            if let Some(c) = h.history.undo(&mut h.buffer) {
                h.selection = Selection::caret(c);
            } else {
                return false;
            }
        }
        Redo => {
            if let Some(c) = h.history.redo(&mut h.buffer) {
                h.selection = Selection::caret(c);
            } else {
                return false;
            }
        }
        _ => return false,
    }
    ensure_visible(h, 40);
    true
}

fn replace_sel(h: &mut BufferHandle, s: &str, label: &'static str) {
    h.history.begin(label, h.selection.head);
    let (a, b) = h.selection.range_offsets(&h.buffer);
    if a != b {
        let text = h.buffer.slice_chars(a, b);
        h.history
            .apply(&mut h.buffer, Edit::Delete { at: a, text });
        h.selection = Selection::caret(h.buffer.point_of(a));
    }
    if !s.is_empty() {
        let at = h.buffer.offset_of(h.selection.head);
        h.history.apply(
            &mut h.buffer,
            Edit::Insert {
                at,
                text: s.to_string(),
            },
        );
        h.selection = Selection::caret(h.buffer.point_of(at + s.chars().count()));
    }
    h.history.commit(h.selection.head);
    h.goal_column = None;
}

fn move_vert(h: &mut BufferHandle, delta: isize) {
    let goal = *h.goal_column.get_or_insert(h.selection.head.column);
    let max = h.buffer.len_lines().saturating_sub(1) as isize;
    let row = (h.selection.head.row as isize + delta).clamp(0, max) as usize;
    h.selection.head = Point {
        row,
        column: goal.min(h.buffer.line_len(row)),
    };
}

fn clamp_point(buf: &Buffer, p: Point) -> Point {
    let row = p.row.min(buf.len_lines().saturating_sub(1));
    Point {
        row,
        column: p.column.min(buf.line_len(row)),
    }
}

fn ensure_visible(h: &mut BufferHandle, vis: usize) {
    let vis = vis.max(1);
    let row = h.selection.head.row;
    if row < h.scroll_row {
        h.scroll_row = row;
    } else if row >= h.scroll_row + vis {
        h.scroll_row = row + 1 - vis;
    }
}

const WELCOME: &str = "\
// rext (dogfood for text-perf., currently zed-like)
//
// Ctrl+P  command palette
// Ctrl+S  save · Ctrl+Z / Ctrl+Shift+Z undo/redo
// Ctrl+A  select all
// Click tree to open · drag dock tabs to split

fn main() {
    println!(\"hello, repose\");
}
";
