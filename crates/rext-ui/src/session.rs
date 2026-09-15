use rext_core::{BufferId, Cmd, DiagnosticsStore, DiffStore, ResolvedTask, SyntaxRegistry, TaskContext, TaskInventory, Workspace};
use repose_core::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub type SessionRef = Rc<Session>;

pub struct Session {
    pub ws: RefCell<Workspace>,
    pub rev: Mutable<u64>,
    pub palette_open: Mutable<bool>,
    pub palette_query: Mutable<String>,
    pub palette_sel: Mutable<usize>,
    pub tree: RefCell<Vec<TreeEntry>>,
    recent: RefCell<Vec<String>>,
    pub diagnostics: RefCell<DiagnosticsStore>,
    pub tasks: RefCell<TaskInventory>,
    pub pending_spawns: RefCell<Vec<ResolvedTask>>,
    pub diffs: RefCell<DiffStore>,
    pub find_open: Mutable<bool>,
    pub find_query: Mutable<String>,
    pub find_match_idx: Mutable<usize>,
    pub syntax: SyntaxRegistry,
    syntax_cache: RefCell<HashMap<BufferId, (u64, Vec<(std::ops::Range<usize>, rext_core::Scope)>)>>,
}

#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

impl Session {
    pub fn bump(&self) {
        self.rev.update(|r| *r = r.wrapping_add(1));
    }

    pub fn record_command(&self, label: &str) {
        let mut recent = self.recent.borrow_mut();
        recent.retain(|r| r != label);
        recent.insert(0, label.to_string());
        recent.truncate(8);
    }

    pub fn recent_commands(&self) -> Vec<String> {
        self.recent.borrow().clone()
    }

    pub fn dispatch(&self, cmd: Cmd, extend: bool) {
        use Cmd::{Copy, Cut, Paste};
        // Clipboard touches platform IO — intercept before `Workspace::exec`.
        match cmd {
            Copy | Cut | Paste => {
                if self.clipboard_cmd(cmd) {
                    self.bump();
                }
                return;
            }
            _ => {}
        }
        let is_save = matches!(cmd, Cmd::Save);
        let changed = self.ws.borrow_mut().exec(cmd, extend);
        if changed {
            self.bump();
            if is_save {
                self.refresh_diffs();
            }
        }
    }

    /// Copy / Cut / Paste via the platform-registered clipboard fns
    /// (`repose_core::clipboard`). Returns true if model state changed.
    fn clipboard_cmd(&self, cmd: Cmd) -> bool {
        use rext_core::{Edit, Selection};
        let mut ws = self.ws.borrow_mut();
        let Some(h) = ws.active_mut() else {
            return false;
        };
        match cmd {
            Cmd::Copy => {
                let ranges = h.selections.ranges(&h.buffer);
                let non_empty: Vec<(usize, usize)> =
                    ranges.into_iter().filter(|(a, b)| a != b).collect();
                if non_empty.is_empty() {
                    ws.status = "nothing selected".into();
                    return true; // status changed → recompose
                }
                let text = non_empty
                    .iter()
                    .map(|(a, b)| h.buffer.slice_chars(*a, *b))
                    .collect::<Vec<_>>()
                    .join("\n");
                repose_core::clipboard::copy_to_clipboard(&text);
                repose_core::clipboard::set_primary_selection(&text);
                ws.status = format!(
                    "copied {} chars ({} selections)",
                    text.chars().count(),
                    non_empty.len()
                );
                true
            }
            Cmd::Cut => {
                let ranges = h.selections.ranges(&h.buffer);
                let non_empty: Vec<(usize, usize)> =
                    ranges.into_iter().filter(|(a, b)| a != b).collect();
                if non_empty.is_empty() {
                    return false;
                }
                let text = non_empty
                    .iter()
                    .map(|(a, b)| h.buffer.slice_chars(*a, *b))
                    .collect::<Vec<_>>()
                    .join("\n");
                repose_core::clipboard::copy_to_clipboard(&text);
                repose_core::clipboard::set_primary_selection(&text);
                let head = h.head();
                h.history.begin("cut", head);
                let mut sorted = non_empty;
                sorted.sort_by(|a, b| b.0.cmp(&a.0));
                let min_start = sorted.iter().map(|(a, _)| *a).min().unwrap_or(0);
                for (a, b) in sorted {
                    let t = h.buffer.slice_chars(a, b);
                    h.history.apply(&mut h.buffer, Edit::Delete { at: a, text: t });
                }
                let np = h.buffer.point_of(min_start);
                h.selections
                    .replace_all(&h.buffer, vec![Selection::caret(np)]);
                h.history.commit(np);
                h.goal_column = None;
                ws.status = "cut".into();
                true
            }
            Cmd::Paste => {
                let Some(text) = repose_core::clipboard::paste_text() else {
                    ws.status = "clipboard empty".into();
                    return true;
                };
                if text.is_empty() {
                    return false;
                }
                // Reuse the insert path (handles selection replace + history).
                drop(ws);
                self.ws.borrow_mut().exec(Cmd::Insert(text), false);
                true
            }
            _ => false,
        }
    }

    pub fn open_file(&self, path: &Path) {
        // Bind first: matching directly on `self.ws.borrow_mut().open_path()`
        // keeps the RefMut alive through the arms, and refresh_diffs()
        // borrows ws again → panic.
        let result = self.ws.borrow_mut().open_path(path);
        match result {
            Ok(_) => {
                self.bump();
                self.refresh_diffs();
            }
            Err(e) => {
                self.ws.borrow_mut().status = format!("{e}");
                self.bump();
            }
        }
    }

    pub fn add_cursor(&self, point: rext_core::Point) {
        use rext_core::{Selection, clamp_point_pub};
        let mut ws = self.ws.borrow_mut();
        let Some(h) = ws.active_mut() else {
            return;
        };
        h.history.seal();
        let point = clamp_point_pub(&h.buffer, point);
        h.selections.add(&h.buffer, Selection::caret(point));
        h.goal_column = None;
        drop(ws);
        self.bump();
    }

    pub fn toggle_dir(&self, idx: usize) {
        let mut tree = self.tree.borrow_mut();
        let Some(entry) = tree.get(idx).cloned() else {
            return;
        };
        if !entry.is_dir {
            return;
        }
        if entry.expanded {
            tree[idx].expanded = false;
            let mut end = idx + 1;
            while end < tree.len() && tree[end].depth > entry.depth {
                end += 1;
            }
            tree.drain(idx + 1..end);
        } else {
            tree[idx].expanded = true;
            let children = list_dir(&entry.path, entry.depth + 1);
            let tail = tree.split_off(idx + 1);
            tree.extend(children);
            tree.extend(tail);
        }
        drop(tree);
        self.bump();
    }

    /// Re-read the root directory, preserving expansion state.
    pub fn refresh_tree(&self) {
        let expanded: std::collections::HashSet<PathBuf> = self
            .tree
            .borrow()
            .iter()
            .filter(|e| e.is_dir && e.expanded)
            .map(|e| e.path.clone())
            .collect();
        let root = self.ws.borrow().root.clone();
        let mut tree = list_dir(&root, 0);
        // Re-expand previously open dirs (one level deep each; deeper state
        // is rebuilt on demand as the user opens).
        let mut i = 0;
        while i < tree.len() {
            if tree[i].is_dir && expanded.contains(&tree[i].path) {
                tree[i].expanded = true;
                let children = list_dir(&tree[i].path, tree[i].depth + 1);
                let tail = tree.split_off(i + 1);
                tree.extend(children);
                tree.extend(tail);
            }
            i += 1;
        }
        *self.tree.borrow_mut() = tree;
        self.bump();
    }

    /// Collapse every expanded directory.
    pub fn collapse_all(&self) {
        let mut tree = self.tree.borrow_mut();
        tree.retain(|e| e.depth == 0);
        for e in tree.iter_mut() {
            e.expanded = false;
        }
        drop(tree);
        self.bump();
    }

    /// Expand ancestors of the active buffer's path and select it.
    /// Returns the tree index to scroll to (unused for now).
    pub fn reveal_active(&self) {
        let path = self
            .ws
            .borrow()
            .active()
            .and_then(|h| h.buffer.path().map(|p| p.to_path_buf()));
        let Some(path) = path else {
            return;
        };
        // Walk up from the file, expanding each ancestor dir row.
        let mut ancestors: Vec<PathBuf> = Vec::new();
        let mut cur = path.parent().map(|p| p.to_path_buf());
        let root = self.ws.borrow().root.clone();
        while let Some(c) = cur {
            if c == root {
                break;
            }
            ancestors.push(c.clone());
            cur = c.parent().map(|p| p.to_path_buf());
            if ancestors.len() > 32 {
                break;
            }
        }
        for anc in ancestors.into_iter().rev() {
            let idx = self
                .tree
                .borrow()
                .iter()
                .position(|e| e.is_dir && e.path == anc && !e.expanded);
            if let Some(i) = idx {
                // expand without bumping per step
                let mut tree = self.tree.borrow_mut();
                tree[i].expanded = true;
                let children = list_dir(&tree[i].path, tree[i].depth + 1);
                let tail = tree.split_off(i + 1);
                tree.extend(children);
                tree.extend(tail);
            }
        }
        self.bump();
    }
}

pub fn list_dir(dir: &Path, depth: usize) -> Vec<TreeEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut v: Vec<_> = rd
        .flatten()
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            !n.starts_with('.') && n != "target"
        })
        .map(|e| {
            let p = e.path();
            TreeEntry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: p.is_dir(),
                path: p,
                depth,
                expanded: false,
            }
        })
        .collect();
    v.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    v
}

pub fn use_session() -> SessionRef {
    remember_with_key("rext:session", || {
        let arg = std::env::args().nth(1).map(PathBuf::from);
        let (root, initial) = match arg {
            Some(p) if p.is_file() => (
                p.parent().map(|x| x.to_path_buf()).unwrap_or_default(),
                Some(p),
            ),
            Some(p) => (p, None),
            None => (std::env::current_dir().unwrap_or_default(), None),
        };
        let tree = list_dir(&root, 0);
        // NOTE: plain `Mutable::new` here, NOT `remember_mutable_with_key`:
        // this closure already runs inside `remember_with_key`'s init, which
        // holds the composer borrow - a nested remember would panic with
        // "RefCell already borrowed". The Session itself is remembered, so
        // these Mutables persist across frames anyway.
        Session {
            rev: Mutable::new(0u64),
            palette_open: Mutable::new(false),
            palette_query: Mutable::new(String::new()),
            palette_sel: Mutable::new(0usize),
            recent: RefCell::new(Vec::new()),
            tree: RefCell::new(tree),
            diagnostics: RefCell::new(DiagnosticsStore::default()),
            tasks: RefCell::new(TaskInventory::default()),
            pending_spawns: RefCell::new(Vec::new()),
            diffs: RefCell::new(DiffStore::default()),
            find_open: Mutable::new(false),
            find_query: Mutable::new(String::new()),
            find_match_idx: Mutable::new(0usize),
            syntax: SyntaxRegistry::new(),
            syntax_cache: RefCell::new(HashMap::new()),
            ws: RefCell::new(Workspace::new(root, initial)),
        }
    })
}

impl Session {
    pub fn highlight_spans(
        &self,
        id: BufferId,
        version: u64,
        lang: Option<&str>,
        text: &str,
    ) -> Vec<(std::ops::Range<usize>, rext_core::Scope)> {
        if let Some((v, spans)) = self.syntax_cache.borrow().get(&id) {
            if *v == version {
                return spans.clone();
            }
        }
        let spans = lang
            .map(|l| self.syntax.highlight(l, text))
            .unwrap_or_default();
        self.syntax_cache
            .borrow_mut()
            .insert(id, (version, spans.clone()));
        spans
    }

    pub fn refresh_tasks(&self) {
        let root = self.ws.borrow().root.clone();
        self.tasks.borrow_mut().refresh(&root);
    }

    pub fn refresh_diffs(&self) {
        let ws = self.ws.borrow();
        let root = ws.root.clone();
        let mut diffs = self.diffs.borrow_mut();
        for (id, handle) in ws.buffers.iter() {
            let base = handle
                .buffer
                .path()
                .and_then(|p| git_head_version(&root, p));
            diffs.update(*id, base, &handle.buffer);
        }
    }

    pub fn task_context(&self) -> TaskContext {
        use rext_core::VariableName;
        let ws = self.ws.borrow();
        let mut cx = TaskContext::default();
        cx.vars.insert(
            VariableName::WorktreeRoot,
            ws.root.display().to_string(),
        );
        if let Some(h) = ws.active() {
            if let Some(p) = h.buffer.path() {
                cx.vars.insert(VariableName::File, p.display().to_string());
                if let Ok(rel) = p.strip_prefix(&ws.root) {
                    cx.vars.insert(
                        VariableName::RelativeFile,
                        rel.display().to_string(),
                    );
                    if let Some(parent) = rel.parent() {
                        cx.vars.insert(
                            VariableName::RelativeDir,
                            parent.display().to_string(),
                        );
                    }
                }
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    cx.vars.insert(VariableName::Filename, name.to_string());
                    cx.vars.insert(
                        VariableName::Stem,
                        name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name).to_string(),
                    );
                }
                if let Some(dir) = p.parent() {
                    cx.vars.insert(
                        VariableName::Dirname,
                        dir.display().to_string(),
                    );
                }
            }
            let (a, b) = h.selections.primary().range_offsets(&h.buffer);
            if a != b {
                cx.vars.insert(
                    VariableName::SelectedText,
                    h.buffer.slice_chars(a, b),
                );
            }
        }
        cx
    }

    pub fn schedule_task(&self, template_idx: usize) {
        let root = self.ws.borrow().root.clone();
        let cx = self.task_context();
        let template = match self.tasks.borrow().templates.get(template_idx).cloned() {
            Some(t) => t,
            None => return,
        };
        let resolved: ResolvedTask =
            self.tasks.borrow_mut().schedule(&template, &root, &cx);
        self.pending_spawns.borrow_mut().push(resolved);
        self.bump();
    }

    pub fn refresh_lints(&self) {
        use crate::panels::problems::lint_buffer;
        use rext_core::DiagnosticSet;
        let ws = self.ws.borrow();
        let mut store = self.diagnostics.borrow_mut();
        store.clear_source("rext-lint");
        for (id, handle) in ws.buffers.iter() {
            let entries = lint_buffer(*id, &handle.buffer);
            if !entries.is_empty() {
                let mut set = store.get(id).cloned().unwrap_or_default();
                set.merge(entries);
                store.set(*id, set);
            }
        }
    }

    pub fn problem_counts(&self) -> (usize, usize) {
        self.diagnostics.borrow().total_counts()
    }

    pub fn problem_items(&self) -> Vec<crate::panels::problems::ProblemItem> {
        let ws = self.ws.borrow();
        let store = self.diagnostics.borrow();
        store
            .all_entries()
            .into_iter()
            .map(|(id, e)| {
                let title = ws
                    .buffers
                    .get(&id)
                    .map(|h| h.buffer.title())
                    .unwrap_or_else(|| "?".into());
                crate::panels::problems::ProblemItem {
                    severity: e.diagnostic.severity,
                    message: e.diagnostic.message.clone(),
                    location: format!(
                        "{}:{}:{}",
                        title,
                        e.range.start.row + 1,
                        e.range.start.column + 1
                    ),
                    target: Some((id, e.range.start)),
                }
            })
            .collect()
    }

    pub fn jump_to(&self, id: rext_core::BufferId, point: rext_core::Point) {
        use rext_core::Selection;
        let mut ws = self.ws.borrow_mut();
        if let Some(idx) = ws.tab_order.iter().position(|b| *b == id) {
            ws.active = idx;
        }
        if let Some(h) = ws.active_mut() {
            h.selections
                .replace_all(&h.buffer, vec![Selection::caret(point)]);
            let vis = 40;
            let row = point.row;
            if row < h.scroll_row {
                h.scroll_row = row;
            } else if row >= h.scroll_row + vis {
                h.scroll_row = row + 1 - vis;
            }
        }
        drop(ws);
        self.bump();
    }

    pub fn jump_to_active(&self, point: rext_core::Point) {
        if let Some(id) = self.ws.borrow().active_id() {
            self.jump_to(id, point);
        }
    }
}

pub fn git_head_version(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let out = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{}", rel.display())])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}
