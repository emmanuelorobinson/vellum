//! The lease state machine (guide §10.2) as a table, and the lease table that holds one per frame. `TEST DATA`.

use vellum_types::{AgentId, NodeId};

use super::{BlockedPairs, LEASE_IDLE_MS, LeaseError, LeaseEvent, LeaseState, Leases};
use crate::ports::{Clock, Timestamp};

const TEAL: &str = "01JA00000000000000000000A1";
const ROSE: &str = "01JA00000000000000000000A2";

fn agent(text: &str) -> AgentId {
    AgentId::new(text).expect("a ULID")
}

fn frame(text: &str) -> NodeId {
    NodeId::new(text).expect("a valid id")
}

fn held(holder: &str, expires_at: i64) -> LeaseState {
    LeaseState::Held {
        agent: agent(holder),
        expires_at: Timestamp(expires_at),
    }
}

/// A clock the test moves by hand.
struct Fixed(std::cell::Cell<i64>);

impl Fixed {
    fn at(ms: i64) -> Self {
        Self(std::cell::Cell::new(ms))
    }
    fn advance(&self, ms: i64) {
        self.0.set(self.0.get() + ms);
    }
}

impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        Timestamp(self.0.get())
    }
}

#[test]
fn lease_idle_expiry_is_sixty_seconds() {
    assert_eq!(LEASE_IDLE_MS, 60_000);
}

/// Every state the machine can be in, crossed with every event, at a time before and after expiry.
#[test]
fn lease_transition_table() {
    use LeaseEvent as E;
    // Teal holds until 50 s. "early" is 10 s (live), "late" is 50 s (expired: expiry is inclusive).
    let early = Timestamp(10_000);
    let late = Timestamp(50_000);
    let teal = || agent(TEAL);
    let rose = || agent(ROSE);
    let free = LeaseState::Free;
    let held_teal = held(TEAL, 50_000);

    type Row = (
        &'static str,
        LeaseState,
        LeaseEvent,
        Timestamp,
        Result<LeaseState, LeaseError>,
    );
    let rows: Vec<Row> = vec![
        // free
        (
            "free claim",
            free.clone(),
            E::Claim(teal()),
            early,
            Ok(held(TEAL, 70_000)),
        ),
        (
            "free activity",
            free.clone(),
            E::Activity(teal()),
            early,
            Ok(LeaseState::Free),
        ),
        (
            "free release",
            free.clone(),
            E::Release(teal()),
            early,
            Err(LeaseError::NotHeld),
        ),
        (
            "free expiry",
            free.clone(),
            E::Expire,
            early,
            Ok(LeaseState::Free),
        ),
        (
            "free stop",
            free.clone(),
            E::Stop,
            early,
            Ok(LeaseState::Free),
        ),
        (
            "free pause all",
            free.clone(),
            E::PauseAll,
            early,
            Ok(LeaseState::Free),
        ),
        // held, live
        (
            "held reclaim by holder extends",
            held_teal.clone(),
            E::Claim(teal()),
            early,
            Ok(held(TEAL, 70_000)),
        ),
        (
            "held claim by other",
            held_teal.clone(),
            E::Claim(rose()),
            early,
            Err(LeaseError::LeasedBy(teal())),
        ),
        (
            "held activity by holder extends",
            held_teal.clone(),
            E::Activity(teal()),
            early,
            Ok(held(TEAL, 70_000)),
        ),
        (
            "held activity by other changes nothing",
            held_teal.clone(),
            E::Activity(rose()),
            early,
            Ok(held_teal.clone()),
        ),
        (
            "held release by holder",
            held_teal.clone(),
            E::Release(teal()),
            early,
            Ok(LeaseState::Free),
        ),
        (
            "held release by other",
            held_teal.clone(),
            E::Release(rose()),
            early,
            Err(LeaseError::LeasedBy(teal())),
        ),
        (
            "held expiry before time",
            held_teal.clone(),
            E::Expire,
            early,
            Ok(held_teal.clone()),
        ),
        (
            "held stop",
            held_teal.clone(),
            E::Stop,
            early,
            Ok(LeaseState::Free),
        ),
        (
            "held pause all",
            held_teal.clone(),
            E::PauseAll,
            early,
            Ok(LeaseState::Free),
        ),
        // held, expired
        (
            "expired claim by holder",
            held_teal.clone(),
            E::Claim(teal()),
            late,
            Ok(held(TEAL, 110_000)),
        ),
        (
            "expired claim by other",
            held_teal.clone(),
            E::Claim(rose()),
            late,
            Ok(held(ROSE, 110_000)),
        ),
        (
            "expired activity does not revive",
            held_teal.clone(),
            E::Activity(teal()),
            late,
            Ok(LeaseState::Free),
        ),
        (
            "expired release",
            held_teal.clone(),
            E::Release(teal()),
            late,
            Err(LeaseError::NotHeld),
        ),
        (
            "expiry at time",
            held_teal.clone(),
            E::Expire,
            late,
            Ok(LeaseState::Free),
        ),
        (
            "expired stop",
            held_teal.clone(),
            E::Stop,
            late,
            Ok(LeaseState::Free),
        ),
        (
            "expired pause all",
            held_teal.clone(),
            E::PauseAll,
            late,
            Ok(LeaseState::Free),
        ),
    ];
    for (name, state, event, now, expected) in rows {
        assert_eq!(state.next(&event, now), expected, "{name}");
    }
}

#[test]
fn lease_holder_is_none_once_expired() {
    let state = held(TEAL, 70_000);
    assert_eq!(state.holder(Timestamp(69_999)), Some(&agent(TEAL)));
    assert_eq!(state.holder(Timestamp(70_000)), None);
    assert_eq!(LeaseState::Free.holder(Timestamp(0)), None);
}

#[test]
fn lease_expires_by_the_injected_clock() {
    let clock = Fixed::at(1_000);
    let mut leases = Leases::default();
    let blocked = BlockedPairs::default();
    let welcome = frame("welcome001");
    leases
        .claim(&welcome, agent(TEAL), &blocked, clock.now())
        .expect("free frame");
    clock.advance(59_999);
    assert_eq!(leases.holder(&welcome, clock.now()), Some(&agent(TEAL)));
    // Activity at 60.999 s pushes expiry to 120.999 s.
    leases.activity(&welcome, &agent(TEAL), clock.now());
    clock.advance(59_999);
    assert_eq!(leases.holder(&welcome, clock.now()), Some(&agent(TEAL)));
    clock.advance(1);
    assert_eq!(leases.holder(&welcome, clock.now()), None);
    assert_eq!(leases.expire(clock.now()), vec![welcome.clone()]);
    assert!(leases.live(clock.now()).next().is_none());
}

#[test]
fn lease_claim_by_another_agent_is_refused_while_live() {
    let mut leases = Leases::default();
    let blocked = BlockedPairs::default();
    let welcome = frame("welcome001");
    leases
        .claim(&welcome, agent(TEAL), &blocked, Timestamp(0))
        .unwrap();
    assert_eq!(
        leases.claim(&welcome, agent(ROSE), &blocked, Timestamp(30_000)),
        Err(LeaseError::LeasedBy(agent(TEAL)))
    );
    leases
        .claim(&welcome, agent(ROSE), &blocked, Timestamp(60_000))
        .expect("Teal's lease expired");
    assert_eq!(
        leases.holder(&welcome, Timestamp(60_000)),
        Some(&agent(ROSE))
    );
}

#[test]
fn lease_stop_frees_the_frame_and_blocks_the_pair_until_resumed() {
    let mut leases = Leases::default();
    let mut blocked = BlockedPairs::default();
    let welcome = frame("welcome001");
    let step = frame("step000001");
    leases
        .claim(&welcome, agent(TEAL), &blocked, Timestamp(0))
        .unwrap();

    assert_eq!(
        leases.stop(&welcome, &mut blocked, Timestamp(1_000)),
        Some(agent(TEAL))
    );
    assert_eq!(leases.holder(&welcome, Timestamp(1_000)), None);
    assert!(blocked.contains(&agent(TEAL), &welcome));

    // Teal cannot take it back; it can take another frame; Rose can take this one.
    assert_eq!(
        leases.claim(&welcome, agent(TEAL), &blocked, Timestamp(2_000)),
        Err(LeaseError::Blocked)
    );
    leases
        .claim(&step, agent(TEAL), &blocked, Timestamp(2_000))
        .unwrap();
    leases
        .claim(&welcome, agent(ROSE), &blocked, Timestamp(2_000))
        .unwrap();
    leases
        .release(&welcome, &agent(ROSE), Timestamp(3_000))
        .unwrap();

    blocked.resume(&agent(TEAL), &welcome);
    assert!(!blocked.contains(&agent(TEAL), &welcome));
    leases
        .claim(&welcome, agent(TEAL), &blocked, Timestamp(4_000))
        .unwrap();
}

#[test]
fn lease_stop_on_a_free_frame_blocks_nobody() {
    let mut leases = Leases::default();
    let mut blocked = BlockedPairs::default();
    assert_eq!(
        leases.stop(&frame("welcome001"), &mut blocked, Timestamp(0)),
        None
    );
    assert!(blocked.is_empty());
}

#[test]
fn lease_resume_agent_clears_all_of_its_blocks() {
    let mut blocked = BlockedPairs::default();
    blocked.block(agent(TEAL), frame("welcome001"));
    blocked.block(agent(TEAL), frame("step000001"));
    blocked.block(agent(ROSE), frame("welcome001"));
    blocked.resume_agent(&agent(TEAL));
    assert!(!blocked.contains(&agent(TEAL), &frame("welcome001")));
    assert!(!blocked.contains(&agent(TEAL), &frame("step000001")));
    assert!(blocked.contains(&agent(ROSE), &frame("welcome001")));
}

#[test]
fn lease_pause_all_frees_every_lease() {
    let mut leases = Leases::default();
    let blocked = BlockedPairs::default();
    leases
        .claim(&frame("welcome001"), agent(TEAL), &blocked, Timestamp(0))
        .unwrap();
    leases
        .claim(&frame("step000001"), agent(ROSE), &blocked, Timestamp(0))
        .unwrap();
    leases.pause_all(Timestamp(10));
    assert!(leases.live(Timestamp(10)).next().is_none());
    // Pausing frees leases but blocks nobody.
    leases
        .claim(&frame("welcome001"), agent(ROSE), &blocked, Timestamp(20))
        .unwrap();
}

#[test]
fn lease_release_by_a_non_holder_is_refused() {
    let mut leases = Leases::default();
    let blocked = BlockedPairs::default();
    let welcome = frame("welcome001");
    leases
        .claim(&welcome, agent(TEAL), &blocked, Timestamp(0))
        .unwrap();
    assert_eq!(
        leases.release(&welcome, &agent(ROSE), Timestamp(1)),
        Err(LeaseError::LeasedBy(agent(TEAL)))
    );
    assert_eq!(
        leases.release(&frame("step000001"), &agent(ROSE), Timestamp(1)),
        Err(LeaseError::NotHeld)
    );
    assert_eq!(leases.holder(&welcome, Timestamp(1)), Some(&agent(TEAL)));
}
