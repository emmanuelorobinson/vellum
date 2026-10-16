//! Session logic every store and host shares (ARCHITECTURE §7.6, ADR-006): previews, token counts by kind,
//! estimated cost, the JSON an event is stored as, rebuilding an engine's history, and recording turn
//! transitions. The store itself is the [`SessionStore`] port.
mod codec;
pub mod pricing;
#[cfg(test)]
mod tests;

pub use codec::{body_from_json, body_to_json, origin_from_json, origin_to_json};

use crate::base64;
use crate::ports::{
    ChatMessage, ChatPart, EventBody, EventSeq, ModelCall, ModelUsage, NewEvent, SessionEvent,
    SessionKind, SessionStore, StoreError, StoredMessage, StoredPart, Timestamp, Tokens,
    TurnTransition, UsageRecord,
};
use crate::turns::{TurnInput, TurnRejection, TurnState, transition};
use pricing::Pricing;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use vellum_types::{Origin, SessionId, TurnId};

/// How much of a step's arguments or result the log keeps, in characters.
pub const PREVIEW_CHARS: usize = 2_000;

/// `value` as compact JSON, cut to `max` characters with a marker naming how much was left out.
pub fn preview(value: &Value, max: usize) -> String {
    let text = value.to_string();
    let total = text.chars().count();
    if total <= max {
        return text;
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}… [{} more characters]", total - max)
}

/// Token counts by kind from what a provider reported (see [`ModelUsage`]).
pub fn tokens_of(usage: &ModelUsage) -> Tokens {
    let cache_read = usage.cached_input_tokens.unwrap_or(0);
    let cache_write = usage.cache_write_tokens.unwrap_or(0);
    Tokens {
        input: usage
            .input_tokens
            .unwrap_or(0)
            .saturating_sub(cache_read)
            .saturating_sub(cache_write),
        output: usage.output_tokens.unwrap_or(0),
        cache_read,
        cache_write,
    }
}

/// The usage event for one model request, with its estimate when `pricing` prices it.
pub fn usage_record(pricing: Option<&Pricing>, model: &str, usage: &ModelUsage) -> UsageRecord {
    let tokens = tokens_of(usage);
    UsageRecord {
        model: model.to_owned(),
        tokens,
        estimate: pricing.and_then(|p| p.estimate(model, tokens)),
    }
}

/// A session's usage, for the usage meter and its popover.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub tokens: Tokens,
    pub by_model: BTreeMap<String, Tokens>,
    /// The sum of the estimates that exist, in micro-USD.
    pub estimate: u64,
    /// Requests with no estimate (model or kind not priced). When non-zero the estimate is a lower bound.
    pub unpriced: u64,
}

pub fn totals<'a>(events: impl IntoIterator<Item = &'a SessionEvent>) -> Totals {
    let mut totals = Totals::default();
    for event in events {
        let EventBody::Usage(usage) = &event.body else {
            continue;
        };
        totals.tokens = totals.tokens.plus(usage.tokens);
        let model = totals.by_model.entry(usage.model.clone()).or_default();
        *model = model.plus(usage.tokens);
        match usage.estimate {
            Some(micro) => totals.estimate = totals.estimate.saturating_add(micro),
            None => totals.unpriced += 1,
        }
    }
    totals
}

/// The checks every store makes before appending: the origin's session and the session's kind. Pictures are
/// checked by the store, which owns the cache.
pub fn check_event(
    session: &SessionId,
    kind: &SessionKind,
    event: &NewEvent,
) -> Result<(), StoreError> {
    if event.origin.session.as_ref() != Some(session) {
        return Err(StoreError::WrongSession);
    }
    if let EventBody::Turn(change) = &event.body
        && event.origin.turn.as_ref() != Some(&change.turn)
    {
        return Err(StoreError::WrongSession);
    }
    if !kind.accepts(&event.body) {
        return Err(StoreError::NotRecorded);
    }
    Ok(())
}

/// Messages as stored: each picture goes into the cache and is named by its hash.
pub fn store_messages(
    store: &dyn SessionStore,
    messages: &[ChatMessage],
) -> Result<Vec<StoredMessage>, StoreError> {
    messages
        .iter()
        .map(|message| {
            let parts = message
                .content
                .iter()
                .map(|part| {
                    Ok(match part {
                        ChatPart::Text(text) => StoredPart::Text(text.clone()),
                        ChatPart::Image { mime, base64: data } => {
                            let bytes = base64::decode(data).ok_or_else(|| {
                                StoreError::Corrupt("a picture is not base64".into())
                            })?;
                            StoredPart::Image {
                                mime: mime.clone(),
                                image: store.put_image(&bytes)?,
                            }
                        }
                        ChatPart::ToolCall(call) => StoredPart::ToolCall {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            arguments: call.arguments.clone(),
                            metadata: call.metadata.clone(),
                        },
                        ChatPart::ToolResult {
                            id,
                            name,
                            result,
                            failed,
                        } => StoredPart::ToolResult {
                            id: id.clone(),
                            name: name.clone(),
                            result: result.clone(),
                            failed: *failed,
                        },
                        ChatPart::Continuation(value) => StoredPart::Continuation(value.clone()),
                    })
                })
                .collect::<Result<_, StoreError>>()?;
            Ok(StoredMessage {
                role: message.role,
                parts,
            })
        })
        .collect()
}

/// The conversation so far, for the next turn's `TurnRequest::history`: every `Messages` event in order, with
/// pictures read back from the cache.
pub fn history(
    store: &dyn SessionStore,
    events: &[SessionEvent],
) -> Result<Vec<ChatMessage>, StoreError> {
    let mut out = vec![];
    for event in events {
        let EventBody::Messages(messages) = &event.body else {
            continue;
        };
        for message in messages {
            let mut content = vec![];
            for part in &message.parts {
                content.push(match part {
                    StoredPart::Text(text) => ChatPart::Text(text.clone()),
                    StoredPart::Image { mime, image } => {
                        let bytes = store
                            .image(image)?
                            .ok_or_else(|| StoreError::MissingImage(image.clone()))?;
                        ChatPart::Image {
                            mime: mime.clone(),
                            base64: base64::encode(&bytes),
                        }
                    }
                    StoredPart::ToolCall {
                        id,
                        name,
                        arguments,
                        metadata,
                    } => ChatPart::ToolCall(ModelCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                        metadata: metadata.clone(),
                    }),
                    StoredPart::ToolResult {
                        id,
                        name,
                        result,
                        failed,
                    } => ChatPart::ToolResult {
                        id: id.clone(),
                        name: name.clone(),
                        result: result.clone(),
                        failed: *failed,
                    },
                    StoredPart::Continuation(value) => ChatPart::Continuation(value.clone()),
                });
            }
            out.push(ChatMessage {
                role: message.role,
                content,
            });
        }
    }
    Ok(out)
}

/// The state a turn reached, from its transitions in the log. `None` when it has none.
pub fn turn_state(events: &[SessionEvent], turn: &TurnId) -> Option<TurnState> {
    events.iter().rev().find_map(|event| match &event.body {
        EventBody::Turn(change) if &change.turn == turn => Some(change.to),
        _ => None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionError {
    Rejected(TurnRejection),
    Store(StoreError),
}
impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(why) => why.fmt(f),
            Self::Store(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for TransitionError {}

/// Moves a turn and appends the one event that says so, with `origin`'s actor (guide §10.1 rule 3). The turn's
/// current state is read from the log (`queued` when it has no transition yet). A rejected input appends nothing.
pub fn record_transition(
    store: &dyn SessionStore,
    origin: &Origin,
    input: TurnInput,
    at: Timestamp,
) -> Result<(TurnState, EventSeq), TransitionError> {
    let session = origin
        .session
        .clone()
        .ok_or(TransitionError::Store(StoreError::WrongSession))?;
    let turn = origin
        .turn
        .clone()
        .ok_or(TransitionError::Store(StoreError::WrongSession))?;
    let events = store
        .events(&session, None)
        .map_err(TransitionError::Store)?;
    let from = turn_state(&events, &turn).unwrap_or(TurnState::Queued);
    let to = transition(from, input, &origin.actor).map_err(TransitionError::Rejected)?;
    let seq = store
        .append(
            &session,
            NewEvent {
                at,
                origin: origin.clone(),
                body: EventBody::Turn(TurnTransition {
                    turn,
                    from,
                    input,
                    to,
                }),
            },
        )
        .map_err(TransitionError::Store)?;
    Ok((to, seq))
}
