//! The small types the ports carry (guide Prompt 10). The behaviour of the ports themselves is checked by the
//! contract suites in `vellum-contracts`.

use vellum_core::ports::{
    ActorId, AssetHash, DirtySet, Extent, OpError, OpErrorKind, Origin, Rect, ScriptError,
    SessionId, SyncCursor, TurnId,
};
use vellum_types::NodeId;

#[test]
fn rectangles_include_their_edges() {
    let a = Rect::new(0.0, 0.0, 10.0, 10.0);
    assert!(
        a.intersects(&Rect::new(10.0, 10.0, 5.0, 5.0)),
        "corners touch"
    );
    assert!(!a.intersects(&Rect::new(10.001, 0.0, 5.0, 5.0)));
    assert!(a.contains_point(10.0, 0.0));
    assert!(!a.contains_point(10.001, 0.0));
    assert!(
        Rect::new(0.0, 5.0, 10.0, 0.0).intersects(&a),
        "a line is a rectangle"
    );
}

#[test]
fn rectangles_grow_and_join() {
    let a = Rect::new(10.0, 10.0, 10.0, 10.0);
    assert_eq!(a.expand(5.0), Rect::new(5.0, 5.0, 20.0, 20.0));
    assert_eq!(a.expand(-100.0).width, 0.0, "shrinking stops at nothing");
    assert_eq!(
        a.union(&Rect::new(0.0, 30.0, 5.0, 5.0)),
        Rect::new(0.0, 10.0, 20.0, 25.0)
    );
    assert!(a.contains_rect(&Rect::new(10.0, 10.0, 10.0, 10.0), 0.0));
    assert!(!a.contains_rect(&Rect::new(9.0, 10.0, 10.0, 10.0), 0.5));
    assert!(a.contains_rect(&Rect::new(9.9, 10.0, 10.0, 10.0), 0.5));
}

#[test]
fn an_unbounded_extent_is_infinite_on_both_axes() {
    assert!(Extent::UNBOUNDED.width.is_infinite() && Extent::UNBOUNDED.height.is_infinite());
    let e = Extent::with_width(40.0);
    assert_eq!((e.width, e.height.is_infinite()), (40.0, true));
}

#[test]
fn an_asset_hash_is_lowercase_hex() {
    assert!(AssetHash::new("00ff").is_ok());
    assert!(AssetHash::new("a".repeat(128)).is_ok());
    for bad in ["", "FF", "xyz", "ab cd", &"a".repeat(129)] {
        assert!(AssetHash::new(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn an_origin_names_who_and_optionally_where() {
    let origin = Origin::new(ActorId::new("agent:teal"))
        .in_session(SessionId::new("s1"))
        .in_turn(TurnId::new("t1"));
    assert_eq!(origin.actor.as_str(), "agent:teal");
    assert_eq!(origin.session.as_ref().map(SessionId::as_str), Some("s1"));
    assert_eq!(origin.turn.as_ref().map(TurnId::as_str), Some("t1"));
    assert_eq!(Origin::new(ActorId::new("x")).session, None);
}

#[test]
fn dirty_sets_say_everything_or_these() {
    assert!(DirtySet::all().all);
    let id = NodeId::new("a").unwrap();
    let some = DirtySet::of([id.clone(), id.clone()]);
    assert!(!some.all && some.nodes.len() == 1);
    assert!(!DirtySet::default().all && DirtySet::default().nodes.is_empty());
}

#[test]
fn errors_speak_plainly() {
    let error = OpError {
        op_index: 2,
        kind: OpErrorKind::UnknownNode(NodeId::new("abc").unwrap()),
    };
    assert_eq!(
        error.to_string(),
        "Change 3 was refused: there is no layer abc."
    );
    assert_eq!(
        OpError {
            op_index: 0,
            kind: OpErrorKind::Empty
        }
        .to_string(),
        "The change has nothing in it."
    );
    assert_eq!(
        ScriptError::Timeout.to_string(),
        "The script ran too long and was stopped."
    );
}

#[test]
fn the_empty_cursor_is_empty() {
    assert!(SyncCursor::empty().0.is_empty());
    assert_eq!(SyncCursor::default(), SyncCursor::empty());
}
