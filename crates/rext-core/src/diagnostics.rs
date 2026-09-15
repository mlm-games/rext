use crate::buffer::{Buffer, BufferId, Point};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    #[default]
    Warning,
    Info,
    Hint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub source: Option<String>,
    pub code: Option<String>,
    pub severity: Severity,
    pub message: String,
    pub group_id: usize,
    pub is_primary: bool,
    pub is_disk_based: bool,
}

impl Default for Diagnostic {
    fn default() -> Self {
        Self {
            source: None,
            code: None,
            severity: Severity::Error,
            message: String::new(),
            group_id: 0,
            is_primary: true,
            is_disk_based: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticEntry {
    pub range: Range<Point>,
    pub diagnostic: Diagnostic,
}

impl DiagnosticEntry {
    pub fn new(range: Range<Point>, diagnostic: Diagnostic) -> Self {
        Self { range, diagnostic }
    }

    pub fn offset_range(&self, buf: &Buffer) -> Range<usize> {
        buf.offset_of(self.range.start)..buf.offset_of(self.range.end)
    }
}

fn compare_entries(a: &DiagnosticEntry, b: &DiagnosticEntry) -> Ordering {
    Ordering::Equal
        .then_with(|| a.range.start.cmp(&b.range.start))
        .then_with(|| b.range.end.cmp(&a.range.end))
        .then_with(|| b.diagnostic.is_primary.cmp(&a.diagnostic.is_primary))
        .then_with(|| {
            a.diagnostic
                .is_disk_based
                .cmp(&b.diagnostic.is_disk_based)
        })
        .then_with(|| a.diagnostic.severity.cmp(&b.diagnostic.severity))
        .then_with(|| a.diagnostic.message.cmp(&b.diagnostic.message))
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticSet {
    entries: Vec<DiagnosticEntry>,
}

impl DiagnosticSet {
    pub fn new(mut entries: Vec<DiagnosticEntry>) -> Self {
        entries.sort_by(compare_entries);
        entries.dedup();
        Self { entries }
    }

    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DiagnosticEntry> {
        self.entries.iter()
    }

    pub fn entries_in_range(
        &self,
        buf: &Buffer,
        range: Range<usize>,
        inclusive: bool,
    ) -> impl Iterator<Item = &DiagnosticEntry> {
        self.entries.iter().filter(move |e| {
            let r = e.offset_range(buf);
            if inclusive {
                r.start <= range.end && r.end >= range.start
            } else {
                r.start < range.end && r.end > range.start
            }
        })
    }

    pub fn merge(&mut self, new_entries: Vec<DiagnosticEntry>) {
        let mut all = std::mem::take(&mut self.entries);
        all.extend(new_entries);
        all.sort_by(compare_entries);
        all.dedup();
        self.entries = all;
    }

    pub fn retain(&mut self, f: impl Fn(&DiagnosticEntry) -> bool) {
        self.entries.retain(f);
    }

    pub fn counts(&self) -> (usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        for e in &self.entries {
            match e.diagnostic.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                _ => {}
            }
        }
        (errors, warnings)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticsStore {
    by_buffer: HashMap<BufferId, DiagnosticSet>,
}

impl DiagnosticsStore {
    pub fn set(&mut self, id: BufferId, set: DiagnosticSet) {
        if set.is_empty() {
            self.by_buffer.remove(&id);
        } else {
            self.by_buffer.insert(id, set);
        }
    }

    pub fn get(&self, id: &BufferId) -> Option<&DiagnosticSet> {
        self.by_buffer.get(id)
    }

    pub fn remove(&mut self, id: &BufferId) {
        self.by_buffer.remove(id);
    }

    pub fn clear_source(&mut self, source: &str) {
        for set in self.by_buffer.values_mut() {
            set.retain(|e| e.diagnostic.source.as_deref() != Some(source));
        }
        self.by_buffer.retain(|_, s| !s.is_empty());
    }

    pub fn total_counts(&self) -> (usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        for set in self.by_buffer.values() {
            let (e, w) = set.counts();
            errors += e;
            warnings += w;
        }
        (errors, warnings)
    }

    pub fn all_entries(&self) -> Vec<(BufferId, &DiagnosticEntry)> {
        let mut out = Vec::new();
        for (id, set) in &self.by_buffer {
            for e in set.iter() {
                out.push((*id, e));
            }
        }
        out.sort_by(|a, b| {
            compare_entries(a.1, b.1).then_with(|| a.0.0.cmp(&b.0.0))
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(sr: usize, sc: usize, er: usize, ec: usize, msg: &str) -> DiagnosticEntry {
        DiagnosticEntry::new(
            Point { row: sr, column: sc }..Point { row: er, column: ec },
            Diagnostic {
                message: msg.into(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn sorts_by_position() {
        let set = DiagnosticSet::new(vec![
            entry(5, 0, 5, 3, "b"),
            entry(1, 0, 1, 3, "a"),
        ]);
        let msgs: Vec<_> = set.iter().map(|e| e.diagnostic.message.as_str()).collect();
        assert_eq!(msgs, &["a", "b"]);
    }

    #[test]
    fn dedups_identical() {
        let set = DiagnosticSet::new(vec![
            entry(1, 0, 1, 3, "a"),
            entry(1, 0, 1, 3, "a"),
        ]);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn range_query() {
        let buf = Buffer::from_str("hello\nworld\n");
        let set = DiagnosticSet::new(vec![
            entry(0, 0, 0, 5, "a"),
            entry(1, 0, 1, 5, "b"),
        ]);
        let hits: Vec<_> = set.entries_in_range(&buf, 0..6, false).collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].diagnostic.message, "a");
    }

    #[test]
    fn store_totals() {
        let mut store = DiagnosticsStore::default();
        store.set(
            BufferId(1),
            DiagnosticSet::new(vec![entry(0, 0, 0, 1, "e")]),
        );
        assert_eq!(store.total_counts(), (1, 0));
        store.clear_source("nope");
        assert_eq!(store.total_counts(), (1, 0));
        store.set(BufferId(1), DiagnosticSet::empty());
        assert_eq!(store.total_counts(), (0, 0));
    }
}
