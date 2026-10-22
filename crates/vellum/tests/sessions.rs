//! TEST DATA. Sessions on disk (guide Prompt 34, ADR-006): MCP clients' tool calls recorded as external sessions,
//! engine turns' usage, messages and ends, and session keys by repository.
mod common;

mod sessions {
    use super::common::{Daemon, design, encode};
    use serde_json::{Value, json};
    use std::path::Path;
    use vellum::mcp::McpServer;
    use vellum::sessions::{SessionLog, doc_key, holds_git, pricing};
    use vellum::tools::Registry;
    use vellum_contracts::fakes::MemorySessions;
    use vellum_core::ports::*;
    use vellum_core::sessions::{history, record_transition, totals};
    use vellum_core::turns::{TurnInput, TurnState};
    use vellum_ipc_local::RpcHandler;
    use vellum_types::{Actor, AgentId, DeviceId, Origin, SessionId, TurnId};

    fn server() -> McpServer {
        let daemon = Daemon::with_scene(&design(), &[]);
        daemon
            .fs
            .seed("work/a/design/app.vellum", &encode(&design()));
        daemon.fs.seed("work/a/other.vellum", &encode(&design()));
        let log = SessionLog::new(Box::new(MemorySessions::new()), pricing())
            .with_repo_check(|dir| dir == Path::new("/work/a"));
        McpServer::new(Registry::standard(), daemon.docs)
            .with_default_cwd(Some("/work/a".into()))
            .with_session_log(Some(log))
    }

    fn rpc(server: &mut McpServer, id: u64, method: &str, params: Value) -> Value {
        server
            .request(
                1,
                json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
            )
            .expect("a reply")
    }

    fn call(server: &mut McpServer, tool: &str, arguments: Value) -> Value {
        let reply = rpc(
            server,
            7,
            "tools/call",
            json!({"name": tool, "arguments": arguments}),
        );
        assert!(reply.get("result").is_some(), "{reply}");
        reply
    }

    fn key(path: &str) -> DocKey {
        DocKey::new("/work/a", path).unwrap()
    }

    #[test]
    fn host_sidebar_reads_recorded_metadata_without_conversation_or_document_content() {
        let mut server = server();
        rpc(
            &mut server,
            1,
            "initialize",
            json!({"protocolVersion":"2025-11-25", "clientInfo":{"name":"TEST DATA client"}}),
        );
        call(
            &mut server,
            "open_document",
            json!({"path":"design/app.vellum"}),
        );
        let reply = rpc(
            &mut server,
            9,
            "vellum/hostState",
            json!({"paths":["/work/a/design/app.vellum"]}),
        );
        let result = &reply["result"];
        assert_eq!(result["sessionsAvailable"], true);
        let rows = result["sessions"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["path"], "/work/a/design/app.vellum");
        assert_eq!(rows[0]["client"], "TEST DATA client");
        assert_eq!(rows[0].as_object().unwrap().len(), 6);
        assert_eq!(
            rows[0]["turn"],
            Value::Null,
            "external tool sessions have no inferred turn outcome"
        );
        assert!(rows[0]["updatedAt"].is_i64());
        assert_eq!(
            rpc(
                &mut server,
                10,
                "vellum/hostState",
                json!({"paths":["/work/another/private.vellum"]})
            )["error"]["code"],
            -32602
        );
    }

    fn steps(store: &dyn SessionStore, id: &SessionId) -> Vec<StepRecord> {
        store
            .events(id, None)
            .unwrap()
            .into_iter()
            .map(|event| match event.body {
                EventBody::Step(step) => step,
                other => panic!("an external session holds steps only, not {other:?}"),
            })
            .collect()
    }

    #[test]
    fn external_mcp_sessions_record_tool_calls_only() {
        let mut server = server();
        rpc(
            &mut server,
            1,
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "Claude Code", "version": "2"}}),
        );
        call(
            &mut server,
            "open_document",
            json!({"path": "design/app.vellum"}),
        );
        call(
            &mut server,
            "insert_nodes",
            json!({"parent": "pageHome01", "nodes": [{"type": "frame", "width": 80, "height": 60,
                   "position": {"type": "absolute", "x": 600, "y": 0},
                   "children": [{"type": "rect", "width": 40, "height": 20}]}]}),
        );
        // The Footer holds a note, which cannot be drawn yet (reports.md): the step is kept without a picture.
        call(
            &mut server,
            "insert_nodes",
            json!({"parent": "frameFoot1", "nodes": [{"type": "rect", "width": 40, "height": 20}]}),
        );
        let failed = call(
            &mut server,
            "insert_nodes",
            json!({"parent": "nowhere", "nodes": [{"type": "rect", "width": 1, "height": 1}]}),
        );
        assert_eq!(failed["result"]["isError"], true);
        call(&mut server, "get_nodes", json!({"ids": ["rectCard01"]}));

        let agent = server.agent(1).unwrap().clone();
        assert!(
            server.steps(1).is_empty(),
            "recorded steps leave the connection"
        );
        let log = server.session_log().unwrap();
        let store = log.store();
        let sessions = store.sessions(&key("design/app.vellum")).unwrap();
        assert_eq!(sessions.len(), 1);
        let meta = &sessions[0];
        assert_eq!(
            meta.kind,
            SessionKind::External {
                client: "Claude Code".into()
            }
        );
        assert_eq!(meta.title, "Claude Code");
        assert_eq!(meta.agent, agent);

        let recorded = steps(store, &meta.id);
        let tools: Vec<_> = recorded.iter().map(|s| s.tool.as_str()).collect();
        assert_eq!(
            tools,
            [
                "open_document",
                "insert_nodes",
                "insert_nodes",
                "insert_nodes",
                "get_nodes"
            ]
        );
        let insert = &recorded[1];
        assert_eq!(insert.outcome, StepOutcome::Done);
        assert!(
            insert.arguments.contains("pageHome01"),
            "{}",
            insert.arguments
        );
        assert_eq!(insert.detail["verb"], "designed");
        let thumbnail = insert
            .thumbnail
            .as_ref()
            .unwrap_or_else(|| panic!("a designed step has a thumbnail: {}", insert.detail));
        let png = store.image(thumbnail).unwrap().expect("cached");
        assert!(png.starts_with(b"\x89PNG"), "the thumbnail is a PNG");
        assert_eq!(recorded[2].outcome, StepOutcome::Done);
        assert_eq!(recorded[2].detail["thumbnail"]["node"], "frameFoot1");
        // The Footer holds a note: notes are left out of pictures, so the frame still gets its thumbnail.
        assert!(
            recorded[2].thumbnail.is_some(),
            "a frame with a note is drawn without it"
        );
        assert_eq!(recorded[3].outcome, StepOutcome::Failed);
        assert!(
            recorded[3].result.contains("nowhere"),
            "{}",
            recorded[3].result
        );
        assert!(recorded[4].thumbnail.is_none());
        for event in store.events(&meta.id, None).unwrap() {
            assert_eq!(
                event.origin.actor,
                Actor::Agent {
                    agent: agent.clone()
                }
            );
            assert_eq!(event.origin.session.as_ref(), Some(&meta.id));
        }
    }

    #[test]
    fn each_document_gets_its_own_session_and_undocumented_first_calls_are_skipped() {
        let mut server = server();
        call(&mut server, "read_skill", json!({}));
        call(
            &mut server,
            "open_document",
            json!({"path": "design/app.vellum"}),
        );
        call(&mut server, "read_skill", json!({}));
        call(
            &mut server,
            "open_document",
            json!({"path": "other.vellum"}),
        );
        let store = server.session_log().unwrap().store();
        let first = store.sessions(&key("design/app.vellum")).unwrap();
        let second = store.sessions(&key("other.vellum")).unwrap();
        assert_eq!((first.len(), second.len()), (1, 1));
        assert_ne!(first[0].id, second[0].id);
        // A call before any document has no session to go to; one after goes with the last document.
        let tools: Vec<_> = steps(store, &first[0].id)
            .into_iter()
            .map(|s| s.tool)
            .collect();
        assert_eq!(tools, ["open_document", "read_skill"]);
        assert_eq!(steps(store, &second[0].id).len(), 1);
        // A client that did not name itself is recorded under the generic name.
        assert_eq!(
            first[0].kind,
            SessionKind::External {
                client: "MCP client".into()
            }
        );
    }

    #[test]
    fn keys_are_the_repository_root_and_the_path_inside_it() {
        let base = std::env::temp_dir().join(format!("vellum-doc-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let repo = base.join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("design/screens")).unwrap();
        let worktree = base.join("tree");
        std::fs::create_dir_all(worktree.join("ui")).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: ../repo/.git/worktrees/tree").unwrap();
        std::fs::create_dir_all(base.join("loose")).unwrap();

        let key = doc_key(&repo.join("design/screens/app.vellum"), &holds_git).unwrap();
        assert_eq!(key.repo_root(), repo.to_string_lossy());
        assert_eq!(key.path(), "design/screens/app.vellum");
        let key = doc_key(&worktree.join("ui/a.vellum"), &holds_git).unwrap();
        assert_eq!(
            (key.repo_root(), key.path()),
            (&*worktree.to_string_lossy(), "ui/a.vellum")
        );
        let key = doc_key(&base.join("loose/b.vellum"), &holds_git).unwrap();
        assert_eq!(key.repo_root(), base.join("loose").to_string_lossy());
        assert_eq!(key.path(), "b.vellum");
        // A folder document is keyed like a file.
        let key = doc_key(&repo.join("design/app.vellum"), &holds_git).unwrap();
        assert_eq!(key.path(), "design/app.vellum");
        assert!(doc_key(Path::new("relative/app.vellum"), &|_| false).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_daemon_keeps_its_sessions_in_its_private_folder() {
        let state = std::env::temp_dir().join(format!("vellum-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&state);
        let log = SessionLog::open(&state).unwrap();
        log.store().put_image(b"TEST DATA").unwrap();
        assert!(state.join("sessions.db").is_file());
        assert_eq!(std::fs::read_dir(state.join("cache")).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&state);
    }

    fn ulid(n: u32) -> String {
        format!("01J{n:023}")
    }

    #[test]
    fn engine_turns_record_usage_messages_and_how_they_ended() {
        let log = SessionLog::new(Box::new(MemorySessions::new()), pricing());
        let store = log.store();
        let session = SessionId::new(ulid(1)).unwrap();
        store
            .create(NewSession {
                id: session.clone(),
                doc: key("design/app.vellum"),
                kind: SessionKind::Engine {
                    engine: "builtin".into(),
                },
                agent: AgentId::new(ulid(2)).unwrap(),
                title: "Pricing card".into(),
                created_at: Timestamp(0),
            })
            .unwrap();
        let turn = TurnId::new(ulid(3)).unwrap();
        let person = Origin::new(Actor::Person {
            device: DeviceId::new(ulid(4)).unwrap(),
        })
        .in_session(session.clone())
        .in_turn(turn.clone());
        let agent = Origin::new(Actor::Agent {
            agent: AgentId::new(ulid(2)).unwrap(),
        })
        .in_session(session.clone())
        .in_turn(turn.clone());
        record_transition(store, &person, TurnInput::Start, Timestamp(1)).unwrap();
        record_transition(store, &agent, TurnInput::NoPlanNeeded, Timestamp(2)).unwrap();

        let messages = vec![
            ChatMessage {
                role: ChatRole::User,
                content: vec![ChatPart::Text("Add a pricing card".into())],
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![
                    ChatPart::Text("Here is how it looks.".into()),
                    ChatPart::Image {
                        mime: "image/png".into(),
                        base64: "iVBORw0KGgo=".into(),
                    },
                ],
            },
        ];
        let model = "claude-opus-5-5";
        let events = [
            TurnEvent::Text("Here is".into()),
            TurnEvent::Usage(ModelUsage {
                input_tokens: Some(1_500),
                output_tokens: Some(100),
                cached_input_tokens: Some(500),
                ..ModelUsage::default()
            }),
            TurnEvent::Usage(ModelUsage {
                input_tokens: Some(10),
                output_tokens: Some(1),
                ..ModelUsage::default()
            }),
            TurnEvent::Messages(messages.clone()),
            TurnEvent::Ended(TurnEnd::Done),
        ];
        let recorded: Vec<_> = events
            .iter()
            .enumerate()
            .map(|(n, event)| {
                log.record_turn_event(&agent, model, event, Timestamp(10 + n as i64))
                    .unwrap()
            })
            .collect();
        assert_eq!(recorded[0], None, "streamed text lives in the messages");
        assert!(recorded[1..].iter().all(Option::is_some));

        let all = store.events(&session, None).unwrap();
        let usage = totals(&all);
        assert_eq!(
            usage.tokens,
            Tokens {
                input: 1_010,
                output: 101,
                cache_read: 500,
                cache_write: 0
            }
        );
        // 1,000 input at $4/M + 100 output at $20/M + 500 cache reads at $0.20/M = 4,000 + 2,000 + 100 µ$;
        // then 10 input + 1 output = 40 + 20 µ$.
        assert_eq!(usage.estimate, 6_160);
        assert_eq!(usage.unpriced, 0);
        assert_eq!(
            history(store, &all).unwrap(),
            messages,
            "the next turn's history"
        );
        let last = all.last().unwrap();
        assert!(matches!(&last.body, EventBody::Turn(t) if t.to == TurnState::Done));
        assert_eq!(last.origin.actor, agent.actor);

        // A failed turn ends failed; a stopped one adds nothing (the person's Stop already moved it).
        let failing = TurnId::new(ulid(5)).unwrap();
        let p2 = person.clone().in_turn(failing.clone());
        let a2 = agent.clone().in_turn(failing);
        record_transition(store, &p2, TurnInput::Start, Timestamp(20)).unwrap();
        record_transition(store, &a2, TurnInput::NoPlanNeeded, Timestamp(21)).unwrap();
        let end = TurnEvent::Ended(TurnEnd::Failed(EngineError::RoundLimit));
        log.record_turn_event(&a2, model, &end, Timestamp(22))
            .unwrap();
        let after = store.events(&session, None).unwrap();
        assert!(
            matches!(&after.last().unwrap().body, EventBody::Turn(t) if t.to == TurnState::Failed)
        );
        let stopped = TurnEvent::Ended(TurnEnd::Stopped);
        assert_eq!(
            log.record_turn_event(&a2, model, &stopped, Timestamp(23))
                .unwrap(),
            None
        );
        assert_eq!(store.events(&session, None).unwrap().len(), after.len());
        // An unpriced model is counted, not guessed.
        let usage = TurnEvent::Usage(ModelUsage {
            input_tokens: Some(5),
            ..ModelUsage::default()
        });
        log.record_turn_event(&agent, "local-model", &usage, Timestamp(30))
            .unwrap();
        assert_eq!(totals(&store.events(&session, None).unwrap()).unpriced, 1);
    }
}
