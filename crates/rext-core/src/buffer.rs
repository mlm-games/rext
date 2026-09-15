use ropey::Rope;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferId(pub u64);

impl BufferId {
    pub fn fresh() -> Self {
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Line/column in **chars** (rope indices). Column may equal line_len (EOL).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

pub struct Buffer {
    pub id: BufferId,
    rope: Rope,
    path: Option<PathBuf>,
    dirty: bool,
    /// Monotonic; DisplayMap / UI cache keys off this.
    version: u64,
}

impl Buffer {
    pub fn empty() -> Self {
        Self {
            id: BufferId::fresh(),
            rope: Rope::new(),
            path: None,
            dirty: false,
            version: 0,
        }
    }

    pub fn from_str(s: &str) -> Self {
        Self {
            id: BufferId::fresh(),
            rope: Rope::from_str(s),
            path: None,
            dirty: false,
            version: 0,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        Ok(Self {
            id: BufferId::fresh(),
            rope: Rope::from_str(&text),
            path: Some(path.to_path_buf()),
            dirty: false,
            version: 0,
        })
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
        self.rope.write_to(&mut f)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        let Some(p) = self.path.clone() else {
            anyhow::bail!("no path");
        };
        self.save_as(p)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".into())
    }

    /// Line without trailing `\n`/`\r\n`.
    pub fn line(&self, row: usize) -> String {
        if row >= self.rope.len_lines() {
            return String::new();
        }
        let mut s: String = self.rope.line(row).chars().collect();
        while matches!(s.chars().last(), Some('\n' | '\r')) {
            s.pop();
        }
        s
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.line(row).chars().count()
    }

    pub fn offset_of(&self, p: Point) -> usize {
        let row = p.row.min(self.len_lines().saturating_sub(1));
        let col = p.column.min(self.line_len(row));
        self.rope.line_to_char(row) + col
    }

    pub fn point_of(&self, offset: usize) -> Point {
        let offset = offset.min(self.len_chars());
        let row = self.rope.char_to_line(offset);
        Point {
            row,
            column: offset - self.rope.line_to_char(row),
        }
    }

    pub fn slice_chars(&self, start: usize, end: usize) -> String {
        let start = start.min(self.len_chars());
        let end = end.min(self.len_chars()).max(start);
        self.rope.slice(start..end).to_string()
    }

    pub(crate) fn insert_raw(&mut self, at: usize, text: &str) {
        self.rope.insert(at.min(self.len_chars()), text);
        self.dirty = true;
        self.version = self.version.wrapping_add(1);
    }

    pub(crate) fn delete_raw(&mut self, start: usize, end: usize) -> String {
        let start = start.min(self.len_chars());
        let end = end.min(self.len_chars()).max(start);
        let removed = self.rope.slice(start..end).to_string();
        if start != end {
            self.rope.remove(start..end);
            self.dirty = true;
            self.version = self.version.wrapping_add(1);
        }
        removed
    }
}
