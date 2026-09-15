use crate::buffer::{Buffer, Point};

/// Inclusive-exclusive char range via anchor + head (Zed/GPUI style).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Point,
    pub head: Point,
}

impl Selection {
    pub fn caret(p: Point) -> Self {
        Self {
            anchor: p,
            head: p,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn range_offsets(&self, buf: &Buffer) -> (usize, usize) {
        let a = buf.offset_of(self.anchor);
        let h = buf.offset_of(self.head);
        (a.min(h), a.max(h))
    }

    pub fn collapse_to_head(self) -> Self {
        Self::caret(self.head)
    }
}
