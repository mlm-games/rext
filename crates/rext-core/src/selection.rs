use crate::buffer::{Buffer, Point};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_SELECTION_ID: AtomicUsize = AtomicUsize::new(1);

fn fresh_id() -> usize {
    NEXT_SELECTION_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionGoal {
    #[default]
    None,
    Column(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub id: usize,
    pub start: Point,
    pub end: Point,
    pub reversed: bool,
    pub goal: SelectionGoal,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            id: fresh_id(),
            start: Point::default(),
            end: Point::default(),
            reversed: false,
            goal: SelectionGoal::None,
        }
    }
}

impl Selection {
    pub fn caret(p: Point) -> Self {
        Self {
            id: fresh_id(),
            start: p,
            end: p,
            reversed: false,
            goal: SelectionGoal::None,
        }
    }

    pub fn head(&self) -> Point {
        if self.reversed {
            self.start
        } else {
            self.end
        }
    }

    pub fn tail(&self) -> Point {
        if self.reversed {
            self.end
        } else {
            self.start
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn set_head(&mut self, head: Point, goal: SelectionGoal) {
        if head < self.tail() {
            if !self.reversed {
                self.end = self.start;
                self.reversed = true;
            }
            self.start = head;
        } else {
            if self.reversed {
                self.start = self.end;
                self.reversed = false;
            }
            self.end = head;
        }
        self.goal = goal;
    }

    pub fn collapse_to(&mut self, point: Point, goal: SelectionGoal) {
        self.start = point;
        self.end = point;
        self.goal = goal;
        self.reversed = false;
    }

    pub fn range_offsets(&self, buf: &Buffer) -> (usize, usize) {
        let a = buf.offset_of(self.start);
        let b = buf.offset_of(self.end);
        (a.min(b), a.max(b))
    }

    pub fn collapse_to_head(self) -> Self {
        Self::caret(self.head())
    }

    pub fn anchor(&self) -> Point {
        self.tail()
    }

    pub fn head_point(&self) -> Point {
        self.head()
    }
}

fn should_merge(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    let overlapping = a_start < b_end && b_start < a_end;
    let same_start = a_start == b_start;
    let cursor_a = a_start == a_end;
    let cursor_b = b_start == b_end;
    let cursor_at_boundary = (cursor_a && (a_start == b_start || a_end == b_end))
        || (cursor_b && (b_start == a_start || b_end == a_end));
    overlapping || same_start || cursor_at_boundary
}

#[derive(Clone, Debug, Default)]
pub struct Selections {
    disjoint: Vec<Selection>,
    pending: Option<Selection>,
}

impl Selections {
    pub fn caret(p: Point) -> Self {
        Self {
            disjoint: vec![Selection::caret(p)],
            pending: None,
        }
    }

    pub fn primary(&self) -> Selection {
        self.pending
            .or_else(|| self.disjoint.last().copied())
            .unwrap_or_default()
    }

    pub fn all(&self) -> Vec<Selection> {
        let mut out = self.disjoint.clone();
        if let Some(p) = self.pending {
            out.push(p);
        }
        out
    }

    pub fn count(&self) -> usize {
        self.disjoint.len() + usize::from(self.pending.is_some())
    }

    pub fn primary_mut(&mut self) -> &mut Selection {
        if self.pending.is_none() && self.disjoint.is_empty() {
            self.disjoint.push(Selection::default());
        }
        self.pending.as_mut().unwrap_or_else(|| {
            self.disjoint.last_mut().expect("non-empty after fill")
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Selection> {
        self.disjoint.iter_mut()
    }

    pub fn set_pending(&mut self, sel: Selection) {
        self.pending = Some(sel);
    }

    pub fn commit_pending(&mut self, buf: &Buffer) {
        if let Some(p) = self.pending.take() {
            self.add(buf, p);
        }
    }

    pub fn clear_pending(&mut self) {
        self.pending = None;
    }

    pub fn add(&mut self, buf: &Buffer, sel: Selection) {
        let (ns, ne) = sel.range_offsets(buf);
        let mut merged_start = sel.start;
        let mut merged_end = sel.end;
        let mut merged_head = sel.head();
        let mut keep: Vec<Selection> = Vec::with_capacity(self.disjoint.len() + 1);
        for existing in self.disjoint.drain(..) {
            let (es, ee) = existing.range_offsets(buf);
            if should_merge(
                buf.offset_of(merged_start),
                buf.offset_of(merged_end),
                es,
                ee,
            ) {
                let _ = (ns, ne);
                if es < buf.offset_of(merged_start) {
                    merged_start = existing.start;
                }
                if ee > buf.offset_of(merged_end) {
                    merged_end = existing.end;
                    merged_head = existing.head();
                }
            } else {
                keep.push(existing);
            }
        }
        let mut merged = Selection {
            id: fresh_id(),
            start: merged_start,
            end: merged_end,
            reversed: false,
            goal: sel.goal,
        };
        merged.set_head(merged_head, sel.goal);
        keep.push(merged);
        keep.sort_by_key(|s| buf.offset_of(s.start));
        self.disjoint = keep;
    }

    pub fn replace_all(&mut self, buf: &Buffer, sels: Vec<Selection>) {
        self.disjoint.clear();
        self.pending = None;
        for s in sels {
            self.add(buf, s);
        }
        if self.disjoint.is_empty() {
            self.disjoint.push(Selection::default());
        }
    }

    pub fn collapse_all(&mut self) {
        for s in &mut self.disjoint {
            let h = s.head();
            s.collapse_to(h, SelectionGoal::None);
        }
        self.pending = None;
        self.dedup_carets();
    }

    fn dedup_carets(&mut self) {
        let mut seen = Vec::new();
        self.disjoint.retain(|s| {
            if seen.contains(&(s.start, s.end)) {
                false
            } else {
                seen.push((s.start, s.end));
                true
            }
        });
        if self.disjoint.is_empty() {
            self.disjoint.push(Selection::default());
        }
    }

    pub fn ranges(&self, buf: &Buffer) -> Vec<(usize, usize)> {
        self.all()
            .iter()
            .map(|s| s.range_offsets(buf))
            .collect()
    }

    pub fn is_empty_single(&self) -> bool {
        self.disjoint.len() == 1
            && self.pending.is_none()
            && self.disjoint[0].is_empty()
    }
}
