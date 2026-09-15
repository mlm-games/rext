use crate::buffer::{Buffer, BufferId};
use std::collections::HashMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    pub buffer_rows: Range<usize>,
    pub base_rows: Range<usize>,
    pub kind: HunkKind,
}

#[derive(Clone, Debug, Default)]
pub struct BufferDiff {
    pub hunks: Vec<DiffHunk>,
    pub buffer_version: u64,
}

impl BufferDiff {
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    pub fn hunks_intersecting_rows(
        &self,
        rows: Range<usize>,
    ) -> impl Iterator<Item = &DiffHunk> {
        self.hunks.iter().filter(move |h| {
            h.buffer_rows.start < rows.end && h.buffer_rows.end > rows.start
        })
    }

    pub fn changed_row_counts(&self) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;
        for h in &self.hunks {
            match h.kind {
                HunkKind::Added => added += h.buffer_rows.len(),
                HunkKind::Modified => {
                    added += h.buffer_rows.len();
                    removed += h.base_rows.len();
                }
                HunkKind::Deleted => removed += h.base_rows.len(),
            }
        }
        (added, removed)
    }
}

pub fn compute_diff(base_text: &str, buffer: &Buffer) -> BufferDiff {
    use imara_diff::{Algorithm, Diff, InternedInput, sources::lines};

    let buffer_text = (0..buffer.len_lines())
        .map(|r| buffer.line(r))
        .collect::<Vec<_>>()
        .join("\n");
    let buffer_text_nl = if buffer_text.ends_with('\n') || buffer_text.is_empty() {
        buffer_text.clone()
    } else {
        format!("{buffer_text}\n")
    };
    let base_nl = if base_text.ends_with('\n') || base_text.is_empty() {
        base_text.to_string()
    } else {
        format!("{base_text}\n")
    };

    if base_nl == buffer_text_nl {
        return BufferDiff {
            hunks: Vec::new(),
            buffer_version: buffer.version(),
        };
    }

    let input = InternedInput::new(lines(base_nl.as_str()), lines(buffer_text_nl.as_str()));
    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    let mut hunks = Vec::new();
    for hunk in diff.hunks() {
        let before = hunk.before.start as usize..hunk.before.end as usize;
        let after = hunk.after.start as usize..hunk.after.end as usize;
        let kind = if before.is_empty() {
            HunkKind::Added
        } else if after.is_empty() {
            HunkKind::Deleted
        } else {
            HunkKind::Modified
        };
        // Deleted hunks anchor to the buffer row where the deletion happened.
        let buffer_rows = if kind == HunkKind::Deleted {
            after.start..after.start
        } else {
            after.clone()
        };
        hunks.push(DiffHunk {
            buffer_rows,
            base_rows: before,
            kind,
        });
    }

    BufferDiff {
        hunks,
        buffer_version: buffer.version(),
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiffStore {
    by_buffer: HashMap<BufferId, BufferDiff>,
    base_texts: HashMap<BufferId, String>,
}

impl DiffStore {
    pub fn update(&mut self, id: BufferId, base_text: Option<String>, buffer: &Buffer) {
        match base_text {
            None => {
                self.by_buffer.remove(&id);
                self.base_texts.remove(&id);
            }
            Some(base) => {
                let changed_base = self.base_texts.get(&id) != Some(&base);
                let stale = self
                    .by_buffer
                    .get(&id)
                    .is_none_or(|d| d.buffer_version != buffer.version());
                if changed_base || stale {
                    self.by_buffer.insert(id, compute_diff(&base, buffer));
                    self.base_texts.insert(id, base);
                }
            }
        }
    }

    pub fn get(&self, id: &BufferId) -> Option<&BufferDiff> {
        self.by_buffer.get(id)
    }

    pub fn remove(&mut self, id: &BufferId) {
        self.by_buffer.remove(id);
        self.base_texts.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_empty() {
        let buf = Buffer::from_str("a\nb\n");
        let d = compute_diff("a\nb\n", &buf);
        assert!(d.is_empty());
    }

    #[test]
    fn added_lines() {
        let buf = Buffer::from_str("a\nb\nc\n");
        let d = compute_diff("a\nc\n", &buf);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].kind, HunkKind::Added);
        assert_eq!(d.hunks[0].buffer_rows, 1..2);
    }

    #[test]
    fn deleted_lines() {
        let buf = Buffer::from_str("a\nc\n");
        let d = compute_diff("a\nb\nc\n", &buf);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].kind, HunkKind::Deleted);
        assert_eq!(d.hunks[0].base_rows, 1..2);
    }

    #[test]
    fn modified_lines() {
        let buf = Buffer::from_str("a\nB\nc\n");
        let d = compute_diff("a\nb\nc\n", &buf);
        assert_eq!(d.hunks.len(), 1);
        assert_eq!(d.hunks[0].kind, HunkKind::Modified);
    }

    #[test]
    fn range_query() {
        let buf = Buffer::from_str("a\nB\nc\n");
        let d = compute_diff("a\nb\nc\n", &buf);
        assert_eq!(d.hunks_intersecting_rows(0..1).count(), 0);
        assert_eq!(d.hunks_intersecting_rows(0..3).count(), 1);
    }

    #[test]
    fn changed_counts() {
        let buf = Buffer::from_str("a\nB\nc\nd\n");
        let d = compute_diff("a\nb\nc\n", &buf);
        let (added, removed) = d.changed_row_counts();
        assert_eq!((added, removed), (2, 1));
    }
}
