//! The turn state machine (guide §10.1), as a table. Pure: no clock, no store.
//!
//! ```text
//! queued ──start──▶ planning ──plan needed──▶ awaiting_plan ──Build──▶ running
//!                      └──no plan needed──────────────────────────────▶ running
//! awaiting_plan ──Edit request──▶ planning
//! running ──permission requested──▶ awaiting_permission ──decided──▶ running
//! running ──done──▶ done      running ──error──▶ failed
//! any non-terminal ──Stop (a person)──▶ stopped
//! ```
//!
//! `done`, `failed` and `stopped` are terminal; Retry creates a new turn. A step that runs longer than
//! [`STUCK_AFTER_MS`] is flagged stuck without changing the state.
use crate::ports::Timestamp;
use std::fmt;
use vellum_types::Actor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TurnState {
    Queued,
    Planning,
    AwaitingPlan,
    Running,
    AwaitingPermission,
    Done,
    Failed,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TurnInput {
    Start,
    PlanNeeded,
    NoPlanNeeded,
    /// The person approved the plan.
    Build,
    EditRequest,
    PermissionRequested,
    /// A permission request was answered (by the person, a policy rule or an engine's auto mode).
    Decided,
    Done,
    Error,
    /// Stop, Stop all or Pause all. Only a person stops a turn.
    Stop,
}

/// The Agent panel's session dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionDot {
    Working,
    NeedsYou,
    Done,
    Failed,
}

impl TurnState {
    pub const ALL: [TurnState; 8] = [
        Self::Queued,
        Self::Planning,
        Self::AwaitingPlan,
        Self::Running,
        Self::AwaitingPermission,
        Self::Done,
        Self::Failed,
        Self::Stopped,
    ];

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Stopped)
    }

    /// The session dot for a turn in this state. A turn stopped by a person keeps what it built and reads
    /// "Stopped by you" beside a done dot: stopping is not a failure.
    pub fn dot(self) -> SessionDot {
        match self {
            Self::Queued | Self::Planning | Self::Running => SessionDot::Working,
            Self::AwaitingPlan | Self::AwaitingPermission => SessionDot::NeedsYou,
            Self::Done | Self::Stopped => SessionDot::Done,
            Self::Failed => SessionDot::Failed,
        }
    }

    /// The name stored in the session log.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Planning => "planning",
            Self::AwaitingPlan => "awaiting_plan",
            Self::Running => "running",
            Self::AwaitingPermission => "awaiting_permission",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.as_str() == name)
    }
}

impl TurnInput {
    pub const ALL: [TurnInput; 10] = [
        Self::Start,
        Self::PlanNeeded,
        Self::NoPlanNeeded,
        Self::Build,
        Self::EditRequest,
        Self::PermissionRequested,
        Self::Decided,
        Self::Done,
        Self::Error,
        Self::Stop,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::PlanNeeded => "plan_needed",
            Self::NoPlanNeeded => "no_plan_needed",
            Self::Build => "build",
            Self::EditRequest => "edit_request",
            Self::PermissionRequested => "permission_requested",
            Self::Decided => "decided",
            Self::Done => "done",
            Self::Error => "error",
            Self::Stop => "stop",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|input| input.as_str() == name)
    }
}

/// Every transition except Stop, which applies to every non-terminal state.
const TABLE: &[(TurnState, TurnInput, TurnState)] = {
    use TurnInput as I;
    use TurnState as S;
    &[
        (S::Queued, I::Start, S::Planning),
        (S::Planning, I::PlanNeeded, S::AwaitingPlan),
        (S::Planning, I::NoPlanNeeded, S::Running),
        (S::AwaitingPlan, I::Build, S::Running),
        (S::AwaitingPlan, I::EditRequest, S::Planning),
        (S::Running, I::PermissionRequested, S::AwaitingPermission),
        (S::AwaitingPermission, I::Decided, S::Running),
        (S::Running, I::Done, S::Done),
        (S::Running, I::Error, S::Failed),
    ]
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnRejection {
    /// The turn has ended; Retry starts a new one.
    Terminal,
    /// The input means nothing in this state.
    NotNow,
    /// Only a person stops a turn (guide §10.1 rule 1).
    NotAPerson,
}
impl fmt::Display for TurnRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Terminal => "the turn has ended",
            Self::NotNow => "the turn cannot do that now",
            Self::NotAPerson => "only a person can stop a turn",
        })
    }
}
impl std::error::Error for TurnRejection {}

/// Where `input` from `actor` takes a turn in `state`.
pub fn transition(
    state: TurnState,
    input: TurnInput,
    actor: &Actor,
) -> Result<TurnState, TurnRejection> {
    if state.is_terminal() {
        return Err(TurnRejection::Terminal);
    }
    if input == TurnInput::Stop {
        return if actor.is_person() {
            Ok(TurnState::Stopped)
        } else {
            Err(TurnRejection::NotAPerson)
        };
    }
    TABLE
        .iter()
        .find(|(from, on, _)| *from == state && *on == input)
        .map(|(_, _, to)| *to)
        .ok_or(TurnRejection::NotNow)
}

/// A step running longer than this is flagged stuck (S10).
pub const STUCK_AFTER_MS: i64 = 120_000;

/// Whether a step that started at `started` and has not finished is stuck at `now`. The flag never changes the
/// turn's state.
pub fn stuck(started: Timestamp, now: Timestamp) -> bool {
    now.0.saturating_sub(started.0) > STUCK_AFTER_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use TurnInput as I;
    use TurnState as S;
    use vellum_types::{AgentId, DeviceId, FileReason, Subsystem};

    fn person() -> Actor {
        Actor::Person {
            device: DeviceId::new("01J00000000000000000000000").unwrap(),
        }
    }
    fn others() -> Vec<Actor> {
        vec![
            Actor::Agent {
                agent: AgentId::new("01J00000000000000000000001").unwrap(),
            },
            Actor::System {
                subsystem: Subsystem::ALL[0],
            },
            Actor::File {
                reason: FileReason::ALL[0],
            },
        ]
    }

    /// The diagram, written out again by hand for every (state × input): `None` means rejected.
    fn expected(state: S, input: I) -> Option<S> {
        match (state, input) {
            (S::Done | S::Failed | S::Stopped, _) => None,
            (_, I::Stop) => Some(S::Stopped),
            (S::Queued, I::Start) => Some(S::Planning),
            (S::Planning, I::PlanNeeded) => Some(S::AwaitingPlan),
            (S::Planning, I::NoPlanNeeded) => Some(S::Running),
            (S::AwaitingPlan, I::Build) => Some(S::Running),
            (S::AwaitingPlan, I::EditRequest) => Some(S::Planning),
            (S::Running, I::PermissionRequested) => Some(S::AwaitingPermission),
            (S::AwaitingPermission, I::Decided) => Some(S::Running),
            (S::Running, I::Done) => Some(S::Done),
            (S::Running, I::Error) => Some(S::Failed),
            _ => None,
        }
    }

    #[test]
    fn every_state_and_input_follows_the_diagram() {
        let mut accepted = 0;
        for state in S::ALL {
            for input in I::ALL {
                let got = transition(state, input, &person()).ok();
                assert_eq!(got, expected(state, input), "{state:?} × {input:?}");
                accepted += usize::from(got.is_some());
            }
        }
        // Nine table rows plus Stop from each of the five non-terminal states.
        assert_eq!(accepted, 14);
    }

    #[test]
    fn rejections_say_why() {
        assert_eq!(
            transition(S::Done, I::Start, &person()),
            Err(TurnRejection::Terminal)
        );
        assert_eq!(
            transition(S::Stopped, I::Stop, &person()),
            Err(TurnRejection::Terminal)
        );
        assert_eq!(
            transition(S::Queued, I::Done, &person()),
            Err(TurnRejection::NotNow)
        );
    }

    #[test]
    fn only_a_person_enters_stopped() {
        for state in S::ALL {
            for input in I::ALL {
                for actor in others() {
                    let to = transition(state, input, &actor);
                    assert_ne!(to, Ok(S::Stopped), "{actor:?} stopped {state:?}");
                    if input == I::Stop && !state.is_terminal() {
                        assert_eq!(to, Err(TurnRejection::NotAPerson));
                    } else {
                        // Every other input is the same whoever sends it.
                        assert_eq!(to.ok(), transition(state, input, &person()).ok());
                    }
                }
            }
        }
    }

    #[test]
    fn terminal_states_have_no_way_out() {
        for state in S::ALL.into_iter().filter(|s| s.is_terminal()) {
            for input in I::ALL {
                assert_eq!(
                    transition(state, input, &person()),
                    Err(TurnRejection::Terminal)
                );
            }
        }
    }

    #[test]
    fn the_session_dot_is_total() {
        let dots: Vec<_> = S::ALL.into_iter().map(S::dot).collect();
        assert_eq!(
            dots,
            [
                SessionDot::Working,
                SessionDot::Working,
                SessionDot::NeedsYou,
                SessionDot::Working,
                SessionDot::NeedsYou,
                SessionDot::Done,
                SessionDot::Failed,
                SessionDot::Done,
            ]
        );
    }

    #[test]
    fn names_round_trip_and_are_distinct() {
        for state in S::ALL {
            assert_eq!(S::parse(state.as_str()), Some(state));
        }
        for input in I::ALL {
            assert_eq!(I::parse(input.as_str()), Some(input));
        }
        assert_eq!(S::parse("Running"), None);
        assert_eq!(I::parse(""), None);
    }

    #[test]
    fn stuck_after_two_minutes_only() {
        let start = Timestamp(1_000);
        assert!(!stuck(start, Timestamp(1_000)));
        assert!(!stuck(start, Timestamp(1_000 + 119_999)));
        assert!(!stuck(start, Timestamp(1_000 + 120_000)));
        assert!(stuck(start, Timestamp(1_000 + 120_001)));
        // A clock that went backwards is not stuck.
        assert!(!stuck(start, Timestamp(0)));
    }
}
