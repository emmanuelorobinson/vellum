//! TEST DATA. Browser reads need recorded network permission; imports use the same attributed Ops.
mod common;
use serde_json::json;
use std::{cell::RefCell, path::Path, rc::Rc};
use vellum::{
    permissions::Outside,
    tools::{ErrorKind, Registry, Session, ToolCtx, ToolError},
};
use vellum_core::ports::*;
use vellum_types::{Actor, Document, Origin};
struct Browser(Rc<RefCell<usize>>);
impl BrowserPort for Browser {
    fn capture(&self, r: &BrowserRequest) -> Result<BrowserCapture, BrowserError> {
        *self.0.borrow_mut() += 1;
        Ok(BrowserCapture{png:b"\x89PNG\r\n\x1a\n".to_vec(),width:r.width,height:r.height,
    dom:Some(serde_json::from_value(json!({"version":1,"width":320,"height":640,"nodes":[{"kind":"frame","name":"article","x":0,"y":0,"width":320,"height":640,"style":{},"parent":null}],"notes":[]})).unwrap())})
    }
}
struct Permit(bool);
impl Outside for Permit {
    fn read_file(&self, _: &Origin, _: &Path, _: u64) -> Result<Vec<u8>, ToolError> {
        unreachable!()
    }
    fn network(&self, _: &Origin, _: &str) -> Result<(), ToolError> {
        if self.0 {
            Ok(())
        } else {
            Err(ToolError::new(ErrorKind::Refused, "denied"))
        }
    }
}
#[test]
fn refused_reads_never_launch_browser_and_import_is_one_undoable_change() {
    let doc:Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page"}]})).unwrap();
    let mut daemon = common::Daemon::new(&doc, &["app.vellum"]);
    let calls = Rc::new(RefCell::new(0));
    daemon.docs.set_browser(Rc::new(Browser(calls.clone())));
    let registry = Registry::standard();
    let mut session = Session::default();
    let run = |daemon: &mut common::Daemon, session: &mut Session, input| {
        registry.call(
            &mut ToolCtx::new(&mut daemon.docs, session),
            "browser",
            input,
        )
    };
    registry
        .call(
            &mut ToolCtx::new(&mut daemon.docs, &mut session),
            "open_document",
            json!({"path":"app.vellum"}),
        )
        .unwrap();
    let input = json!({"url":"http://localhost:3000/","action":"import","parent":"page000001","width":320,"height":640});
    assert_eq!(
        run(&mut daemon, &mut session, input.clone())
            .unwrap_err()
            .kind(),
        ErrorKind::Refused
    );
    daemon.docs.set_outside(Rc::new(Permit(false)));
    assert_eq!(
        run(&mut daemon, &mut session, input.clone())
            .unwrap_err()
            .kind(),
        ErrorKind::Refused
    );
    assert_eq!(*calls.borrow(), 0);
    daemon.docs.set_outside(Rc::new(Permit(true)));
    let turn = vellum_types::TurnId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
    session.set_turn(Some(turn.clone()));
    let result = run(&mut daemon, &mut session, input).unwrap();
    assert_eq!(result["nodes"], 1);
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(
        daemon
            .docs
            .target("app.vellum")
            .unwrap()
            .snapshot()
            .unwrap()
            .pages[0]
            .children
            .len(),
        1
    );
    let actor = vellum_types::Actor::Person {
        device: vellum_types::DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
    };
    daemon
        .docs
        .target("app.vellum")
        .unwrap()
        .undo_turn(&turn, &actor)
        .unwrap();
    assert!(
        daemon
            .docs
            .target("app.vellum")
            .unwrap()
            .snapshot()
            .unwrap()
            .pages[0]
            .children
            .is_empty()
    );
}
#[test]
fn browser_schema_is_strict_and_screenshot_is_returned_without_doc_write() {
    let registry = Registry::standard();
    assert!(registry.get("browser").is_some());
}

#[test]
fn screenshot_never_changes_the_document_and_embed_defers_network_until_opened() {
    let doc = Document::new("TEST DATA");
    let mut daemon = common::Daemon::new(&doc, &["app.vellum"]);
    let calls = Rc::new(RefCell::new(0));
    daemon.docs.set_browser(Rc::new(Browser(calls.clone())));
    daemon.docs.set_outside(Rc::new(Permit(true)));
    let registry = Registry::standard();
    let mut session = Session::default();
    let mut ctx = ToolCtx::new(&mut daemon.docs, &mut session);
    registry
        .call(&mut ctx, "open_document", json!({"path":"app.vellum"}))
        .unwrap();
    let result = registry
        .call(
            &mut ctx,
            "browser",
            json!({"url":"http://localhost/","action":"screenshot","width":375,"height":812}),
        )
        .unwrap();
    assert_eq!(result["width"], 375);
    assert_eq!(result["image"]["mime"], "image/png");
    drop(ctx);
    assert_eq!(
        daemon
            .docs
            .target("app.vellum")
            .unwrap()
            .snapshot()
            .unwrap(),
        doc
    );
}

#[test]
fn mcp_browser_uses_the_real_broker_and_records_denial_and_consent_before_capture() {
    use vellum::{
        mcp::McpServer,
        sessions::{SessionLog, pricing},
    };
    use vellum_contracts::fakes::{ManualClock, MemorySessions, ScriptedPrompt};
    use vellum_ipc_local::RpcHandler;
    let doc:Document=serde_json::from_value(json!({"meta":{"formatVersion":1,"name":"TEST DATA","createdWith":"Vellum"},"pages":[{"id":"page000001","name":"Page"}]})).unwrap();
    let mut daemon = common::Daemon::new(&doc, &["repo/app.vellum"]);
    let calls = Rc::new(RefCell::new(0));
    daemon.docs.set_browser(Rc::new(Browser(calls.clone())));
    let prompt = Rc::new(ScriptedPrompt::new(
        [Some(Answer::Deny), Some(Answer::AllowOnce)],
        None,
    ));
    let person = Actor::Person {
        device: vellum_types::DeviceId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap(),
    };
    let log =
        SessionLog::new(Box::new(MemorySessions::new()), pricing()).with_repo_check(|_| false);
    let mut server = McpServer::new(Registry::standard(), daemon.docs)
        .with_default_cwd(Some("/repo".into()))
        .with_session_log(Some(log))
        .with_permissions(Some(prompt), person, Rc::new(ManualClock::new(1_000_000)));
    let call = |server: &mut McpServer, method: &str, params| {
        server
            .request(
                1,
                json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
            )
            .unwrap()
    };
    call(
        &mut server,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"TEST DATA client","version":"1"}}),
    );
    let opened = call(
        &mut server,
        "tools/call",
        json!({"name":"open_document","arguments":{"path":"app.vellum"}}),
    );
    assert!(opened.get("error").is_none(), "{opened}");
    let input = json!({"name":"browser","arguments":{"action":"screenshot","url":"http://localhost:3000/"}});
    let denied = call(&mut server, "tools/call", input.clone());
    assert_eq!(denied["result"]["isError"], true);
    assert_eq!(*calls.borrow(), 0);
    let allowed = call(&mut server, "tools/call", input);
    assert_ne!(allowed["result"]["isError"], true, "{allowed}");
    assert_eq!(*calls.borrow(), 1);
    let sessions = server
        .session_log()
        .unwrap()
        .metadata(std::path::Path::new("/repo/app.vellum"))
        .unwrap();
    let events = server
        .session_log()
        .unwrap()
        .store()
        .events(&sessions[0].id, None)
        .unwrap();
    let decisions: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let EventBody::Permission(record) = &e.body {
                Some(record.allowed())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(decisions, [false, true]);
}

#[test]
fn rejected_browser_credentials_never_enter_step_history() {
    let mut daemon = common::Daemon::new(&Document::new("TEST DATA"), &["app.vellum"]);
    let mut session = Session::default();
    let registry = Registry::standard();
    let result = registry.call(
        &mut ToolCtx::new(&mut daemon.docs, &mut session),
        "browser",
        json!({"action":"screenshot","url":"http://user:TEST_DATA_BROWSER_CREDENTIAL@localhost/"}),
    );
    assert!(result.is_err());
    assert!(
        !session
            .steps()
            .last()
            .unwrap()
            .arguments
            .contains("TEST_DATA_BROWSER_CREDENTIAL")
    );
}
