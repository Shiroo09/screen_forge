//! Shared undo ordering. Port of `ScreenForge/Gif/Editing/EditHistory.cs`.
//!
//! The document and the scene keep their own undo stacks; this only knows which
//! one is due next, so Ctrl+Z always undoes the user's most recent action
//! regardless of whether it was a frame or a drawing edit.

/// Which subsystem an undoable action belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditScope {
    /// Frame edits (delete, duplicate, crop, rotate).
    Frames,
    /// Drawing edits (add, move, delete an object).
    Annotation,
}

pub struct EditHistory {
    undo: Vec<EditScope>,
    redo: Vec<EditScope>,
    limit: usize,
}

impl EditHistory {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The scope due to be undone next, if any.
    pub fn next_undo(&self) -> Option<EditScope> {
        self.undo.last().copied()
    }

    /// The scope due to be redone next, if any.
    pub fn next_redo(&self) -> Option<EditScope> {
        self.redo.last().copied()
    }

    /// Records a new action and invalidates the redo history.
    pub fn record(&mut self, scope: EditScope) {
        self.undo.push(scope);
        self.redo.clear();

        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    /// Moves the next action from undo to redo.
    pub fn pop_undo(&mut self) -> Option<EditScope> {
        let scope = self.undo.pop()?;
        self.redo.push(scope);
        Some(scope)
    }

    /// Moves the next action from redo back to undo.
    pub fn pop_redo(&mut self) -> Option<EditScope> {
        let scope = self.redo.pop()?;
        self.undo.push(scope);
        Some(scope)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new(100)
    }
}
