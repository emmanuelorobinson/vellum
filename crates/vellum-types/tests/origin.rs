//! Who made a change (guide Invariant 2): ids outside the file are ULIDs, and an `Actor` is a person's device,
//! an agent, a file change or a part of the system. History, presence and undo read this and nothing else.

use std::str::FromStr;

use serde_json::json;
use vellum_types::{Actor, AgentId, DeviceId, FileReason, Origin, SessionId, Subsystem, TurnId};

const ULID: &str = "01JA2B3C4D5E6F7G8H9JKMNPQR";

#[test]
fn a_ulid_id_round_trips_through_text_and_json() {
    let id = SessionId::new(ULID).expect("a ULID");
    assert_eq!(id.as_str(), ULID);
    assert_eq!(SessionId::from_str(ULID).expect("parse"), id);
    let json = serde_json::to_string(&id).expect("serialise");
    assert_eq!(json, format!("\"{ULID}\""));
    assert_eq!(serde_json::from_str::<SessionId>(&json).expect("read"), id);
}

#[test]
fn a_ulid_id_is_26_crockford_characters_and_fits_in_128_bits() {
    for bad in [
        "",
        "01JA2B3C4D5E6F7G8H9JKMNPQ",   // 25
        "01JA2B3C4D5E6F7G8H9JKMNPQRS", // 27
        "01JA2B3C4D5E6F7G8H9JKMNPQI",  // I is not Crockford
        "01JA2B3C4D5E6F7G8H9JKMNPQU",  // nor U
        "01ja2b3c4d5e6f7g8h9jkmnpqr",  // lower case is not the canonical spelling
        "81JA2B3C4D5E6F7G8H9JKMNPQR",  // more than 128 bits
    ] {
        assert!(TurnId::new(bad).is_err(), "{bad:?} should be refused");
        assert!(
            serde_json::from_value::<TurnId>(json!(bad)).is_err(),
            "{bad:?} should not read"
        );
    }
    assert!(AgentId::new("7ZZZZZZZZZZZZZZZZZZZZZZZZZ").is_ok());
    assert!(DeviceId::new(ULID).is_ok());
}

#[test]
fn an_actor_prints_with_its_kind() {
    let cases = [
        (
            Actor::Person {
                device: DeviceId::new(ULID).expect("id"),
            },
            json!({"kind": "person", "device": ULID}),
        ),
        (
            Actor::Agent {
                agent: AgentId::new(ULID).expect("id"),
            },
            json!({"kind": "agent", "agent": ULID}),
        ),
        (
            Actor::File {
                reason: FileReason::ExternalChange,
            },
            json!({"kind": "file", "reason": "external-change"}),
        ),
        (
            Actor::System {
                subsystem: Subsystem::Sync,
            },
            json!({"kind": "system", "subsystem": "sync"}),
        ),
    ];
    for (actor, expected) in cases {
        assert_eq!(serde_json::to_value(&actor).expect("serialise"), expected);
        assert_eq!(
            serde_json::from_value::<Actor>(expected).expect("read"),
            actor
        );
    }
    assert!(serde_json::from_value::<Actor>(json!({"kind": "robot"})).is_err());
    assert!(
        serde_json::from_value::<Actor>(json!({"kind": "file", "reason": {"merge": null}}))
            .is_err(),
        "a reason has one spelling"
    );
}

#[test]
fn an_actor_can_tell_people_from_agents() {
    let person = Actor::Person {
        device: DeviceId::new(ULID).expect("id"),
    };
    let agent = Actor::Agent {
        agent: AgentId::new(ULID).expect("id"),
    };
    assert!(person.is_person() && !person.is_agent());
    assert!(agent.is_agent() && !agent.is_person());
    let file = Actor::File {
        reason: FileReason::Merge,
    };
    assert!(!file.is_person() && !file.is_agent());
}

#[test]
fn an_origin_always_has_an_actor_and_may_have_a_session_and_turn() {
    let actor = Actor::Agent {
        agent: AgentId::new(ULID).expect("id"),
    };
    let origin = Origin::new(actor.clone());
    assert_eq!(origin.actor, actor);
    assert_eq!(origin.session, None);
    assert_eq!(origin.turn, None);
    assert_eq!(
        serde_json::to_value(&origin).expect("serialise"),
        json!({"actor": {"kind": "agent", "agent": ULID}})
    );

    let session = SessionId::new(ULID).expect("id");
    let turn = TurnId::new(ULID).expect("id");
    let origin = origin.in_session(session.clone()).in_turn(turn.clone());
    assert_eq!(origin.session, Some(session));
    assert_eq!(origin.turn, Some(turn));
    let back: Origin =
        serde_json::from_value(serde_json::to_value(&origin).expect("serialise")).expect("read");
    assert_eq!(back, origin);
}
