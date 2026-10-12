//! Who made a change (guide Invariant 2).
//!
//! Every transaction carries an [`Origin`]: an [`Actor`] and, for agent work, the session and turn. History,
//! presence and undo read it and nothing else, and the UI draws agents as rounded squares and people as circles
//! from the actor alone. Ids outside the file are ULIDs (guide §9).

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::defaults::serde_as_string;
use crate::enums::{FileReason, Subsystem};
use crate::ids::IdError;

const CROCKFORD: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn check_ulid(text: &str) -> Result<(), IdError> {
    let valid = text.len() == 26
        && text.chars().all(|c| CROCKFORD.contains(c))
        && text.as_bytes()[0] <= b'7';
    if valid { Ok(()) } else { Err(IdError::NotUlid) }
}

macro_rules! ulid_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, JsonSchema, TS)]
        #[schemars(with = "String")]
        #[ts(export, type = "string")]
        pub struct $name(String);

        impl $name {
            /// Checks that `id` is a ULID and wraps it.
            pub fn new(id: impl Into<String>) -> Result<Self, IdError> {
                let id = id.into();
                check_ulid(&id)?;
                Ok(Self(id))
            }

            /// The id as text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(id: &str) -> Result<Self, IdError> {
                Self::new(id)
            }
        }

        serde_as_string!($name);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

ulid_type!(
    /// An agent session.
    SessionId
);
ulid_type!(
    /// A turn of an agent session; "Undo this turn" reverts by it.
    TurnId
);
ulid_type!(
    /// An agent: a built-in one ("Teal") or an external client ("Claude Code").
    AgentId
);
ulid_type!(
    /// A person's device. A person is known by the device they edit from; there are no accounts locally.
    DeviceId
);

/// Who made a change.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Actor {
    /// A person, at a device.
    Person {
        /// The device.
        device: DeviceId,
    },
    /// An agent.
    Agent {
        /// Which one.
        agent: AgentId,
    },
    /// The file itself: it changed on disk, was merged or restored.
    File {
        /// Why.
        reason: FileReason,
    },
    /// A part of Vellum acting on nobody's behalf.
    System {
        /// Which part.
        subsystem: Subsystem,
    },
}

impl Actor {
    /// Whether a person made the change. People are drawn as circles.
    pub fn is_person(&self) -> bool {
        matches!(self, Self::Person { .. })
    }

    /// Whether an agent made the change. Agents are drawn as rounded squares.
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }
}

/// Where a change came from. There is no default: an origin cannot exist without an actor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Origin {
    /// Who.
    pub actor: Actor,
    /// The agent session, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
    /// The turn, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<TurnId>,
}

impl Origin {
    /// An origin with an actor and nothing else.
    pub fn new(actor: Actor) -> Self {
        Self {
            actor,
            session: None,
            turn: None,
        }
    }

    /// The same origin inside a session.
    #[must_use]
    pub fn in_session(mut self, session: SessionId) -> Self {
        self.session = Some(session);
        self
    }

    /// The same origin inside a turn.
    #[must_use]
    pub fn in_turn(mut self, turn: TurnId) -> Self {
        self.turn = Some(turn);
        self
    }
}
