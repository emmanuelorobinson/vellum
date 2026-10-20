//! TEST DATA: comments follow the same attributed, checked write path as design edits.
use vellum_core::{comments, ops};
use vellum_types::{
    Actor, DeviceId, Document, NodeId, Point, comments::CommentRequest, sync::Role,
};
fn actor(n: u8) -> Actor {
    Actor::Person {
        device: DeviceId::new(format!("{:026}", n)).unwrap(),
    }
}
fn create() -> CommentRequest {
    CommentRequest::Create {
        thread: NodeId::new("thread1").unwrap(),
        message: NodeId::new("message1").unwrap(),
        anchor: None,
        page: None,
        point: Point { x: 12.0, y: 34.0 },
        text: "TEST DATA: review this".into(),
    }
}
#[test]
fn comments_are_attributed_bounded_and_role_checked() {
    let doc = Document::new("TEST DATA");
    assert!(comments::transaction(&doc, actor(1), Role::Viewer, create(), 5).is_err());
    let tx = comments::transaction(&doc, actor(1), Role::Commenter, create(), 5).unwrap();
    assert_eq!(tx.origin.actor, actor(1));
    let mut ids = NoIds;
    let next = ops::plan(&doc, &tx, &mut ids).unwrap().document;
    assert_eq!(next.comments.len(), 1);
    let resolve = CommentRequest::Resolve {
        thread: NodeId::new("thread1").unwrap(),
        resolved: true,
    };
    assert!(comments::transaction(&next, actor(2), Role::Commenter, resolve.clone(), 6).is_err());
    assert!(comments::transaction(&next, actor(1), Role::Commenter, resolve.clone(), 6).is_ok());
    assert!(comments::transaction(&next, actor(2), Role::Editor, resolve, 6).is_ok());
    let mut invalid = create();
    if let CommentRequest::Create { text, .. } = &mut invalid {
        *text = " ".into();
    }
    assert!(comments::transaction(&doc, actor(1), Role::Editor, invalid, 5).is_err());
    assert!(comments::transaction(&next, actor(1), Role::Editor, create(), 5).is_err());
}
struct NoIds;
impl vellum_core::ports::IdGen for NoIds {
    fn node_id(&mut self) -> NodeId {
        panic!("comment IDs are supplied, never random")
    }
}
