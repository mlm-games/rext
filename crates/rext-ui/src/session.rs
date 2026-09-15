use rext_core::{Buffer, Cmd, Workspace};
use repose_core::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub type SessionRef = Rc<Session>;

pub struct Session {
    pub ws: RefCell<Workspace>,
    pub rev: Mutable<u64>,
    pub palette_open: Mutable<bool>,
    pub palette_query: Mutable<String>,
    pub tree: RefCell<Vec<TreeEntry>>,
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

    pub fn dispatch(&self, cmd: Cmd, extend: bool) {
        let changed = self.ws.borrow_mut().exec(cmd, extend);
        if changed {
            self.bump();
        }
    }

    pub fn open_file(&self, path: &Path) {
        match self.ws.borrow_mut().open_path(path) {
            Ok(_) => self.bump(),
            Err(e) => {
                self.ws.borrow_mut().status = format!("{e}");
                self.bump();
            }
        }
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
        Session {
            rev: remember_mutable_with_key("rext:rev", || 0u64),
            palette_open: remember_mutable_with_key("rext:pal", || false),
            palette_query: remember_mutable_with_key("rext:pq", || String::new()),
            tree: RefCell::new(tree),
            ws: RefCell::new(Workspace::new(root, initial)),
        }
    })
}
