use crate::buffer::{Buffer, Point};

#[derive(Clone, Debug)]
pub enum Edit {
    Insert { at: usize, text: String },
    Delete { at: usize, text: String },
}

impl Edit {
    fn inverse(&self) -> Self {
        match self {
            Edit::Insert { at, text } => Edit::Delete {
                at: *at,
                text: text.clone(),
            },
            Edit::Delete { at, text } => Edit::Insert {
                at: *at,
                text: text.clone(),
            },
        }
    }

    fn apply(&self, buf: &mut Buffer) {
        match self {
            Edit::Insert { at, text } => buf.insert_raw(*at, text),
            Edit::Delete { at, text } => {
                let _ = buf.delete_raw(*at, *at + text.chars().count());
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Tx {
    label: &'static str,
    edits: Vec<Edit>,
    cursor_before: Point,
    cursor_after: Point,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Tx>,
    redo: Vec<Tx>,
    open: Option<Tx>,
}

impl History {
    pub fn begin(&mut self, label: &'static str, cursor: Point) {
        if self.open.is_some() {
            // nested begin: seal previous without cursor_after update.
            if let Some(mut t) = self.open.take() {
                if !t.edits.is_empty() {
                    t.cursor_after = cursor;
                    self.redo.clear();
                    self.undo.push(t);
                }
            }
        }
        self.open = Some(Tx {
            label,
            edits: vec![],
            cursor_before: cursor,
            cursor_after: cursor,
        });
    }

    pub fn apply(&mut self, buf: &mut Buffer, edit: Edit) {
        let tx = self.open.as_mut().expect("History::apply without begin");
        edit.apply(buf);
        tx.edits.push(edit);
    }

    pub fn commit(&mut self, cursor_after: Point) {
        let Some(mut tx) = self.open.take() else {
            return;
        };
        if tx.edits.is_empty() {
            return;
        }
        tx.cursor_after = cursor_after;
        self.redo.clear();

        if let Some(prev) = self.undo.last_mut() {
            if prev.label == tx.label && coalescable(tx.label) {
                if can_coalesce_inserts(prev, &tx) || can_coalesce_deletes(prev, &tx) {
                    prev.edits.append(&mut tx.edits);
                    prev.cursor_after = tx.cursor_after;
                    return;
                }
            }
        }
        self.undo.push(tx);
    }

    pub fn cancel(&mut self, buf: &mut Buffer) -> Option<Point> {
        let tx = self.open.take()?;
        for e in tx.edits.iter().rev() {
            e.inverse().apply(buf);
        }
        Some(tx.cursor_before)
    }

    pub fn undo(&mut self, buf: &mut Buffer) -> Option<Point> {
        if self.open.is_some() {
            return self.cancel(buf);
        }
        let tx = self.undo.pop()?;
        for e in tx.edits.iter().rev() {
            e.inverse().apply(buf);
        }
        let c = tx.cursor_before;
        self.redo.push(tx);
        Some(c)
    }

    pub fn redo(&mut self, buf: &mut Buffer) -> Option<Point> {
        let tx = self.redo.pop()?;
        for e in &tx.edits {
            e.apply(buf);
        }
        let c = tx.cursor_after;
        self.undo.push(tx);
        Some(c)
    }

    /// Force undo boundary (arrow keys, click, etc.).
    pub fn seal(&mut self) {
        if let Some(t) = self.undo.last_mut() {
            if coalescable(t.label) {
                t.label = "sealed";
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.open.as_ref().is_some_and(|t| !t.edits.is_empty())
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn coalescable(label: &str) -> bool {
    matches!(label, "type" | "backspace" | "delete")
}

fn can_coalesce_inserts(prev: &Tx, next: &Tx) -> bool {
    if prev.label != "type" || next.label != "type" || next.edits.len() != 1 {
        return false;
    }
    let Some(Edit::Insert { at: pa, text: pt }) = prev.edits.last() else {
        return false;
    };
    let Some(Edit::Insert { at: na, text: _ }) = next.edits.first() else {
        return false;
    };
    *pa + pt.chars().count() == *na
}

fn can_coalesce_deletes(prev: &Tx, next: &Tx) -> bool {
    if prev.label != next.label || next.edits.len() != 1 {
        return false;
    }
    if !matches!(prev.label, "backspace" | "delete") {
        return false;
    }
    matches!(
        (prev.edits.last(), next.edits.first()),
        (Some(Edit::Delete { .. }), Some(Edit::Delete { .. }))
    )
}
