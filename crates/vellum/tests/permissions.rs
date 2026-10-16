//! TEST DATA. Outside the canvas on a real disk (guide Prompt 35, Invariant 11): the broker's permit is the only
//! way the binary reads, writes or runs for an agent, links are asked about where they lead, and canvas edits
//! never ask.
mod common;

mod permissions {
    use super::common::{Daemon, design, encode};
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use vellum::mcp::McpServer;
    use vellum::permissions::{
        OutsideError, read_action, read_file, run_action, run_command, write_action, write_file,
    };
    use vellum::sessions::{SessionLog, pricing};
    use vellum::tools::Registry;
    use vellum_contracts::fakes::{ManualClock, MemorySessions, ScriptedPrompt};
    use vellum_contracts::suites::model::block_on;
    use vellum_core::permissions::{Broker, PermissionSettings, Permit};
    use vellum_core::ports::*;
    use vellum_ipc_local::RpcHandler;
    use vellum_types::{Actor, AgentId, DeviceId, Origin, SessionId};

    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    /// A repository on disk, removed at the end.
    struct Repo(PathBuf);
    impl Repo {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "v-perm-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(root.join("repo/.git")).unwrap();
            std::fs::create_dir_all(root.join("outside")).unwrap();
            std::fs::write(root.join("repo/DESIGN.md"), "TEST DATA guidelines").unwrap();
            std::fs::write(root.join("outside/secret.txt"), "TEST DATA secret").unwrap();
            Self(root.canonicalize().unwrap())
        }
        fn repo(&self) -> PathBuf {
            self.0.join("repo")
        }
    }
    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn session() -> SessionId {
        SessionId::new("01J000000000000000000000QS").unwrap()
    }
    fn origin() -> Origin {
        Origin::new(Actor::Agent {
            agent: AgentId::new("01J000000000000000000000QA").unwrap(),
        })
        .in_session(session())
    }

    struct Host {
        store: Rc<MemorySessions>,
        prompt: Rc<ScriptedPrompt>,
        broker: Broker,
    }
    impl Host {
        fn new(repo: &Path, answer: Answer) -> Self {
            let store = Rc::new(MemorySessions::new());
            store
                .create(NewSession {
                    id: session(),
                    doc: DocKey::new(repo.to_string_lossy(), "design/app.vellum").unwrap(),
                    kind: SessionKind::Engine {
                        engine: "builtin".into(),
                    },
                    agent: AgentId::new("01J000000000000000000000QA").unwrap(),
                    title: "TEST DATA".into(),
                    created_at: Timestamp(0),
                })
                .unwrap();
            let prompt = Rc::new(ScriptedPrompt::always(answer));
            let broker = Broker::new(
                PermissionSettings::default(),
                store.clone(),
                Rc::new(ManualClock::new(1)),
                Some(prompt.clone()),
                Actor::Person {
                    device: DeviceId::new("01J000000000000000000000QD").unwrap(),
                },
            );
            Self {
                store,
                prompt,
                broker,
            }
        }
        fn permit(&self, action: Action) -> Result<Permit, Refusal> {
            block_on(self.broker.request(PermissionRequest {
                origin: origin(),
                action,
                engine: Some("builtin".into()),
                engine_verdict: None,
            }))
        }
    }

    #[test]
    fn a_permit_reads_writes_and_runs_exactly_what_it_names() {
        let disk = Repo::new();
        let host = Host::new(&disk.repo(), Answer::AllowOnce);
        let design = disk.repo().join("DESIGN.md");

        let permit = host.permit(read_action(&design).unwrap()).unwrap();
        assert_eq!(permit.kind(), PermissionKind::ReadRepo);
        assert_eq!(read_file(permit, &design).unwrap(), b"TEST DATA guidelines");

        // A permit for one file does not read another.
        let permit = host.permit(read_action(&design).unwrap()).unwrap();
        let secret = disk.0.join("outside/secret.txt");
        assert!(matches!(
            read_file(permit, &secret),
            Err(OutsideError::NotPermitted)
        ));

        let theme = disk.repo().join("theme.css");
        let permit = host.permit(write_action(&theme).unwrap()).unwrap();
        assert_eq!(permit.kind(), PermissionKind::WriteRepo);
        write_file(permit, &theme, b":root{}").unwrap();
        assert_eq!(std::fs::read(&theme).unwrap(), b":root{}");

        #[cfg(unix)]
        {
            let args = vec!["TEST DATA".to_owned()];
            let permit = host
                .permit(run_action("echo", &args, &disk.repo()).unwrap())
                .unwrap();
            let out = run_command(permit, "echo", &args, &disk.repo()).unwrap();
            assert_eq!(out.stdout, b"TEST DATA\n");
            let permit = host
                .permit(run_action("echo", &args, &disk.repo()).unwrap())
                .unwrap();
            assert!(matches!(
                run_command(permit, "echo", &["other".into()], &disk.repo()),
                Err(OutsideError::NotPermitted)
            ));
        }

        let decisions = host
            .store
            .events(&session(), None)
            .unwrap()
            .into_iter()
            .filter(|e| matches!(e.body, EventBody::Permission(_)))
            .count();
        assert_eq!(
            decisions,
            host.prompt.asked().len(),
            "every use was decided and recorded"
        );
    }

    #[test]
    fn a_denied_action_never_happens() {
        let disk = Repo::new();
        let host = Host::new(&disk.repo(), Answer::Deny);
        let theme = disk.repo().join("theme.css");
        let refusal = host.permit(write_action(&theme).unwrap()).unwrap_err();
        assert_eq!(refusal.record.answer, Answer::Deny);
        assert!(!theme.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_repository_is_asked_about_where_it_leads() {
        let disk = Repo::new();
        let host = Host::new(&disk.repo(), Answer::AllowOnce);
        let link = disk.repo().join("notes.txt");
        std::os::unix::fs::symlink(disk.0.join("outside/secret.txt"), &link).unwrap();
        let permit = host.permit(read_action(&link).unwrap()).unwrap();
        assert_eq!(permit.kind(), PermissionKind::ReadOutside);
        assert_eq!(host.prompt.asked()[0].kind, PermissionKind::ReadOutside);
        assert_eq!(
            host.prompt.asked()[0].offered,
            [Answer::AllowOnce, Answer::Deny],
            "outside reads are asked every time"
        );
        // A link inside a folder that points out: a new file there is outside too.
        std::os::unix::fs::symlink(disk.0.join("outside"), disk.repo().join("out")).unwrap();
        let action = write_action(&disk.repo().join("out/new.txt")).unwrap();
        assert_eq!(
            host.permit(action).unwrap().kind(),
            PermissionKind::WriteOutside
        );
    }

    // ---- canvas edits never prompt ---------------------------------------------------------------------------

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
        assert_eq!(reply["result"]["isError"], false, "{tool}: {reply}");
        reply
    }

    #[test]
    fn canvas_edits_never_prompt() {
        let daemon = Daemon::with_scene(&design(), &[]);
        daemon
            .fs
            .seed("work/a/design/app.vellum", &encode(&design()));
        let log = SessionLog::new(Box::new(MemorySessions::new()), pricing())
            .with_repo_check(|dir| dir == Path::new("/work/a"));
        let mut server = McpServer::new(Registry::standard(), daemon.docs)
            .with_default_cwd(Some("/work/a".into()))
            .with_session_log(Some(log));
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
        let inserted = call(
            &mut server,
            "insert_nodes",
            json!({"parent": "pageHome01", "nodes": [{"type": "frame", "width": 80, "height": 60,
                   "position": {"type": "absolute", "x": 600, "y": 0}}]}),
        );
        let id = inserted["result"]["structuredContent"]["created"][0]
            .as_str()
            .expect("the new frame's id")
            .to_owned();
        call(
            &mut server,
            "update_nodes",
            json!({"updates": [{"id": "rectCard01", "set": {"opacity": 0.5}}]}),
        );
        call(
            &mut server,
            "set_variables",
            json!({"variables": [{"name": "space/8", "type": "number", "values": [{"value": 32}]}]}),
        );
        call(&mut server, "delete_nodes", json!({"ids": [id]}));

        let log = server.session_log().unwrap();
        let store = log.store();
        let sessions = store
            .sessions(&DocKey::new("/work/a", "design/app.vellum").unwrap())
            .unwrap();
        assert_eq!(sessions.len(), 1);
        let events = store.events(&sessions[0].id, None).unwrap();
        assert!(events.len() >= 4, "the edits were recorded as steps");
        assert!(
            events.iter().all(|e| matches!(e.body, EventBody::Step(_))),
            "no permission was asked for or recorded for canvas edits"
        );
    }

    /// The tools reach documents through the daemon's targets only; anything outside the canvas has to go through
    /// `vellum::permissions`, which needs a permit. So no tool may touch the disk, the network or a process itself.
    #[test]
    fn tools_reach_outside_the_canvas_only_through_a_permit() {
        let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tools");
        let forbidden = [
            "std::fs",
            "std::process",
            "std::net",
            "File::",
            "OpenOptions",
            "Command::",
            "TcpStream",
            "UdpSocket",
        ];
        let mut found = vec![];
        for entry in std::fs::read_dir(&tools).unwrap() {
            let path = entry.unwrap().path();
            let source = std::fs::read_to_string(&path).unwrap();
            for word in forbidden {
                if source.contains(word) {
                    found.push(format!("{} uses {word}", path.display()));
                }
            }
        }
        assert!(found.is_empty(), "{found:#?}");
    }
}
