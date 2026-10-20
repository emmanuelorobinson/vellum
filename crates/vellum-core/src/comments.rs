//! Comment policy and checked Op construction. No clock, storage, network or generated identities.
use crate::ops::{Op, Shell, Transaction};
use std::collections::BTreeMap;
use vellum_types::{Actor, Document, NodeId, Origin, comments::*, sync::Role};
pub const MAX_THREADS: usize = 1024;
pub const MAX_REPLIES: usize = 256;
pub const MAX_COMMENT_BYTES: usize = 8192;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentError {
    Forbidden,
    Invalid,
    Missing,
    Duplicate,
    Limit,
}
fn text_ok(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= MAX_COMMENT_BYTES && !s.contains('\0')
}
pub fn valid(threads: &BTreeMap<NodeId, CommentThread>) -> bool {
    threads.len() <= MAX_THREADS
        && threads.values().all(|t| {
            t.author.is_person()
                && t.point.x.is_finite()
                && t.point.y.is_finite()
                && t.created_ms >= 0
                && !t.messages.is_empty()
                && t.messages.len() <= MAX_REPLIES
                && t.resolved == t.resolved_by.is_some()
                && t.resolved_by.as_ref().is_none_or(Actor::is_person)
                && t.messages
                    .values()
                    .all(|m| m.author.is_person() && m.at_ms >= 0 && text_ok(&m.text))
        })
}
pub fn transaction(
    doc: &Document,
    actor: Actor,
    role: Role,
    request: CommentRequest,
    now_ms: i64,
) -> Result<Transaction, CommentError> {
    if role == Role::Viewer || !actor.is_person() {
        return Err(CommentError::Forbidden);
    }
    if now_ms < 0 {
        return Err(CommentError::Invalid);
    }
    let mut shell = Shell::of(doc);
    match request {
        CommentRequest::Create {
            thread,
            message,
            anchor,
            page,
            point,
            text,
        } => {
            if shell.comments.contains_key(&thread) {
                return Err(CommentError::Duplicate);
            }
            if shell.comments.len() >= MAX_THREADS {
                return Err(CommentError::Limit);
            }
            if !text_ok(&text) || !point.x.is_finite() || !point.y.is_finite() {
                return Err(CommentError::Invalid);
            }
            if anchor
                .as_ref()
                .is_some_and(|id| crate::ops::find(doc, id).is_none())
            {
                return Err(CommentError::Missing);
            }
            if page
                .as_ref()
                .is_some_and(|id| !doc.pages.iter().any(|p| &p.id == id))
            {
                return Err(CommentError::Missing);
            }
            shell.comments.insert(
                thread,
                CommentThread {
                    author: actor.clone(),
                    anchor,
                    page,
                    point,
                    created_ms: now_ms,
                    resolved: false,
                    resolved_by: None,
                    messages: BTreeMap::from([(
                        message,
                        CommentMessage {
                            author: actor.clone(),
                            at_ms: now_ms,
                            text,
                        },
                    )]),
                },
            );
        }
        CommentRequest::Reply {
            thread,
            message,
            text,
        } => {
            let t = shell
                .comments
                .get_mut(&thread)
                .ok_or(CommentError::Missing)?;
            if t.messages.contains_key(&message) {
                return Err(CommentError::Duplicate);
            }
            if t.resolved || !text_ok(&text) {
                return Err(CommentError::Invalid);
            }
            if t.messages.len() >= MAX_REPLIES {
                return Err(CommentError::Limit);
            }
            t.messages.insert(
                message,
                CommentMessage {
                    author: actor.clone(),
                    at_ms: now_ms,
                    text,
                },
            );
        }
        CommentRequest::Resolve { thread, resolved } => {
            let t = shell
                .comments
                .get_mut(&thread)
                .ok_or(CommentError::Missing)?;
            if t.author != actor && role != Role::Editor {
                return Err(CommentError::Forbidden);
            }
            t.resolved = resolved;
            t.resolved_by = resolved.then(|| actor.clone());
        }
    }
    Ok(Transaction::new(
        Origin::new(actor),
        vec![Op::SetShell {
            shell: Box::new(shell),
        }],
    ))
}
