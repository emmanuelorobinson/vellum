//! The live target: an editor has the file open and its engine owns the document, so transactions are forwarded
//! to it and the canvas updates as they land (ARCHITECTURE §7.1; guide §7 Ownership). Editors that open the same
//! file afterwards attach to that engine and follow it; they never write the file themselves.

use std::fmt;

use vellum_core::ops::{Applied, Transaction};
use vellum_core::ports::{ChangeSet, DocumentTarget, ScriptValue, TargetError, TargetMode};
use vellum_types::Document;

/// Which editor connection: one per open editor tab or window, named by the host.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EditorId(pub String);

impl fmt::Display for EditorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The daemon's end of one editor's connection (relayed by its host; never a socket in a webview). A broken
/// connection answers [`TargetError::Gone`].
pub trait EditorLink {
    /// The document in the editor's engine, unsaved changes included.
    fn snapshot(&mut self) -> Result<Document, TargetError>;
    /// Applies `tx` in the editor's engine: one transaction, one undo step, through its Guard.
    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError>;
    /// Draws with the editor's renderer.
    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError>;
    /// Asks the editor to save; answers with the document it wrote.
    fn save(&mut self) -> Result<Document, TargetError>;
    /// Whether the editor has changes the file does not.
    fn is_dirty(&mut self) -> Result<bool, TargetError>;
    /// For an attached editor: the writer's engine changed; show `document`.
    fn follow(&mut self, changes: &ChangeSet, document: &Document) -> Result<(), TargetError>;
    /// For an attached editor: it is now the writer. Its engine already holds what it was following.
    fn promote(&mut self) -> Result<(), TargetError>;
}

/// The role an editor gets when it opens a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Its engine owns the document; transactions for the file go to it.
    Writer,
    /// Another editor is the writer; this one shows that engine's document and sends its edits there.
    Attached,
    /// Nobody can say who the writer is (the daemon is not reachable), so the file opens read-only.
    ReadOnly,
}

impl Role {
    /// What an editor does when it cannot reach the daemon: it cannot learn whether another editor holds the
    /// file, and two writers would overwrite each other, so it opens read-only (guide §7 Ownership).
    pub const OFFLINE: Role = Role::ReadOnly;
}

/// Forwards to the writer and keeps attached editors following it.
pub struct LiveTarget {
    writer: (EditorId, Box<dyn EditorLink>),
    attached: Vec<(EditorId, Box<dyn EditorLink>)>,
    /// The writer stopped answering. The daemon drops the target and falls back to the file.
    lost: bool,
}

impl LiveTarget {
    /// A target forwarding to `writer`.
    pub fn new(id: EditorId, writer: Box<dyn EditorLink>) -> Self {
        Self {
            writer: (id, writer),
            attached: Vec::new(),
            lost: false,
        }
    }

    /// The writer.
    pub fn writer(&self) -> &EditorId {
        &self.writer.0
    }

    /// The attached editors, in the order they opened.
    pub fn attached(&self) -> impl Iterator<Item = &EditorId> {
        self.attached.iter().map(|(id, _)| id)
    }

    /// Whether `editor` is the writer or attached.
    pub fn has(&self, editor: &EditorId) -> bool {
        self.writer.0 == *editor || self.attached.iter().any(|(id, _)| id == editor)
    }

    /// Whether the writer stopped answering.
    pub fn is_lost(&self) -> bool {
        self.lost
    }

    /// Attaches another editor to the writer's engine.
    pub fn attach(&mut self, id: EditorId, link: Box<dyn EditorLink>) {
        self.attached.push((id, link));
    }

    /// Removes an editor. When it was the writer the first attached editor takes over; `None` when nobody is left.
    pub fn detach(mut self, editor: &EditorId) -> Option<Self> {
        if self.writer.0 != *editor {
            self.attached.retain(|(id, _)| id != editor);
            return Some(self);
        }
        while !self.attached.is_empty() {
            let (id, mut link) = self.attached.remove(0);
            if link.promote().is_ok() {
                self.writer = (id, link);
                self.lost = false;
                return Some(self);
            }
        }
        None
    }

    fn check<T>(&mut self, result: Result<T, TargetError>) -> Result<T, TargetError> {
        if matches!(result, Err(TargetError::Gone)) {
            self.lost = true;
        }
        result
    }
}

impl DocumentTarget for LiveTarget {
    fn mode(&self) -> TargetMode {
        TargetMode::Live
    }

    fn snapshot(&mut self) -> Result<Document, TargetError> {
        let result = self.writer.1.snapshot();
        self.check(result)
    }

    fn transact(&mut self, tx: Transaction) -> Result<Applied, TargetError> {
        let result = self.writer.1.transact(tx);
        let applied = self.check(result)?;
        if !self.attached.is_empty() {
            let document = self.snapshot()?;
            // A follower that went away is dropped; the writer's change stands.
            self.attached
                .retain_mut(|(_, link)| link.follow(&applied.changes, &document).is_ok());
        }
        Ok(applied)
    }

    fn render(&mut self, target: &str, scale: f64) -> Result<ScriptValue, TargetError> {
        let result = self.writer.1.render(target, scale);
        self.check(result)
    }

    fn save(&mut self) -> Result<(), TargetError> {
        self.save_returning().map(|_| ())
    }

    fn is_dirty(&mut self) -> Result<bool, TargetError> {
        let result = self.writer.1.is_dirty();
        self.check(result)
    }
}

impl LiveTarget {
    /// Saves through the writer and returns what it wrote.
    pub fn save_returning(&mut self) -> Result<Document, TargetError> {
        let result = self.writer.1.save();
        self.check(result)
    }
}
