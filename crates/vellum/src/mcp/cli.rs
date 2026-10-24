//! Native CLI turns: engines stay in the client, while attribution, reservations and logging stay with
//! the daemon's existing document targets. These methods are never advertised as model tools.
use super::*;
use vellum_core::{
    guard::{AgentControl, LeaseChange},
    parallel::{self, LaneEnd, LaneRecord, Mode, ParallelEvent},
    plans::{Placeholder, Plan},
    ports::*,
    sessions::record_transition,
    turns::TurnInput,
};
use vellum_types::{Actor, AgentId, DeviceId, NodeId, Origin, SessionId, TurnId};

type Result<T> = std::result::Result<T, (i64, String)>;
fn error(e: impl std::fmt::Display) -> (i64, String) {
    (-32000, e.to_string())
}
fn text<'a>(p: &'a Value, key: &str) -> Result<&'a str> {
    p[key]
        .as_str()
        .ok_or_else(|| error(format!("Missing {key}")))
}
#[derive(Clone)]
pub(super) struct Run {
    document: String,
    turn: TurnId,
    session: SessionId,
    agent: AgentId,
    person: Actor,
    model: String,
    owner: u64,
    plan: Option<Plan>,
    ended: bool,
    prepared: bool,
}
impl Run {
    pub(super) fn is_ended(&self) -> bool {
        self.ended
    }
    fn origin(&self) -> Origin {
        Origin::new(Actor::Agent {
            agent: self.agent.clone(),
        })
        .in_session(self.session.clone())
        .in_turn(self.turn.clone())
    }
    fn person(&self) -> Origin {
        Origin::new(self.person.clone())
            .in_session(self.session.clone())
            .in_turn(self.turn.clone())
    }
}
impl McpServer {
    fn cli_append(&self, run: &Run, body: EventBody) -> Result<()> {
        let log = self.log.as_ref().ok_or_else(|| {
            error("Session recording unavailable; repair the session store before running an agent")
        })?;
        log.store()
            .append(
                &run.session,
                NewEvent {
                    at: self.docs.now(),
                    origin: run.origin(),
                    body,
                },
            )
            .map_err(error)?;
        Ok(())
    }
    pub(super) fn cli_turn_action(&mut self, conn: u64, params: &Value) -> Result<Value> {
        let action = text(params, "action")?;
        if action == "control" {
            let file = text(params, "document")?;
            self.session(conn);
            let path = ToolCtx::new(
                &mut self.docs,
                self.sessions.get_mut(&conn).expect("session"),
            )
            .resolve(file)
            .map_err(error)?;
            let run = self
                .cli_runs
                .values()
                .find(|r| {
                    r.document == path
                        && params["turn"].as_str() == Some(r.turn.as_str())
                        && params["agent"].as_str() == Some(r.agent.as_str())
                })
                .cloned()
                .ok_or_else(|| error("CLI lane is no longer connected"))?;
            let paused = self.docs.view(&path).map_err(error)?.state.paused;
            return Ok(
                json!({"stop":run.ended||paused.all||paused.agents.contains(&run.agent)||paused.stopped.contains(&run.agent),"all":paused.all}),
            );
        }
        if action == "turn" {
            return self.cli_begin(conn, params);
        }
        let run = self
            .cli_runs
            .get(&conn)
            .filter(|r| !r.ended)
            .cloned()
            .ok_or_else(|| error("No active CLI turn on this connection"))?;
        match action {
            "usage" => {
                let n = |key| params["usage"][key].as_u64();
                let usage = ModelUsage {
                    input_tokens: n("input_tokens"),
                    output_tokens: n("output_tokens"),
                    reasoning_tokens: n("reasoning_tokens"),
                    cached_input_tokens: n("cached_input_tokens"),
                    cache_write_tokens: n("cache_write_tokens"),
                };
                self.log
                    .as_ref()
                    .ok_or_else(|| error("Session recording unavailable"))?
                    .record_turn_event(
                        &run.origin(),
                        &run.model,
                        &TurnEvent::Usage(usage),
                        self.docs.now(),
                    )
                    .map_err(error)?;
                Ok(json!({"recorded":true}))
            }
            "messages" => {
                let messages: Vec<ChatMessage> = params["messages"]
                    .as_array()
                    .ok_or_else(|| error("Messages must be an array"))?
                    .iter()
                    .map(decode_message)
                    .collect::<Result<_>>()?;
                self.log
                    .as_ref()
                    .ok_or_else(|| error("Session recording unavailable"))?
                    .record_turn_event(
                        &run.origin(),
                        &run.model,
                        &TurnEvent::Messages(messages),
                        self.docs.now(),
                    )
                    .map_err(error)?;
                Ok(json!({"recorded":true}))
            }
            "stop" => {
                let group: Vec<_> = self
                    .cli_runs
                    .iter()
                    .filter(|(_, r)| r.turn == run.turn)
                    .map(|(id, r)| (*id, r.clone()))
                    .collect();
                for (id, lane) in group {
                    self.docs.control_all(&AgentControl::Stop(lane.agent));
                    self.cli_runs.get_mut(&id).expect("run").ended = true;
                }
                self.docs.unreserve(&run.document, |p| p.turn == run.turn);
                let log = self
                    .log
                    .as_ref()
                    .ok_or_else(|| error("Session recording unavailable"))?;
                record_transition(log.store(), &run.person(), TurnInput::Stop, self.docs.now())
                    .map_err(error)?;
                Ok(json!({"stopped":true}))
            }
            "end" => {
                let failed = params["failed"] == true;
                self.cli_finish(conn, failed)?;
                Ok(json!({"ended":true}))
            }
            "plan" => {
                let doc = self
                    .docs
                    .target(&run.document)
                    .and_then(|t| t.snapshot())
                    .map_err(error)?;
                let proposed: crate::orchestrate::ProposeInput =
                    serde_json::from_value(params["plan"].clone()).map_err(error)?;
                let plan = crate::orchestrate::resolve(&doc, proposed).map_err(error)?;
                self.cli_append(
                    &run,
                    EventBody::Plan(vellum_core::plans::PlanEvent::Proposed(plan.clone())),
                )?;
                self.cli_runs.get_mut(&conn).expect("run").plan = Some(plan);
                Ok(
                    json!({"proposed":true,"message":"The plan is recorded. End this planning run; the CLI will dispatch the approved task."}),
                )
            }
            "prepare" => self.cli_prepare(conn, params),
            "clear" => {
                self.docs.unreserve(&run.document, |p| p.turn == run.turn);
                Ok(json!({"cleared":true}))
            }
            _ => Err(error("Unknown CLI action")),
        }
    }
    fn cli_begin(&mut self, conn: u64, p: &Value) -> Result<Value> {
        if self.cli_runs.get(&conn).is_some_and(|r| !r.ended) {
            return Err(error("A CLI turn is already active"));
        }
        let file = text(p, "document")?;
        self.session(conn);
        let cwd = self.sessions[&conn].cwd.clone();
        let path = ToolCtx::new(
            &mut self.docs,
            self.sessions.get_mut(&conn).expect("session"),
        )
        .resolve(file)
        .map_err(error)?;
        self.docs.target(&path).map_err(error)?;
        let agent = AgentId::new(self.docs.ulid()).expect("ULID");
        let model = text(p, "model")?.to_owned();
        let engine = text(p, "engine")?;
        if !matches!(engine, "builtin" | "claude-code" | "codex") {
            return Err(error("Unknown engine"));
        }
        let resumed = if let Some(id) = p["session"].as_str() {
            let log = self
                .log
                .as_ref()
                .ok_or_else(|| error("Sessions are unavailable"))?;
            let meta = log
                .metadata(std::path::Path::new(&path))
                .map_err(error)?
                .into_iter()
                .find(|m| m.id.as_str() == id && matches!(m.kind, SessionKind::Engine { .. }))
                .ok_or_else(|| error("No such engine session for this document"))?;
            if self
                .cli_runs
                .values()
                .any(|r| r.session == meta.id && !r.ended)
            {
                return Err(error("This session already has a running turn"));
            }
            Some(meta)
        } else {
            None
        };
        let run = if let Some(turn) = p["join"].as_str() {
            // Join only a running owner connection for this document, never an arbitrary stored turn.
            let primary = self
                .cli_runs
                .iter()
                .find(|(id, r)| {
                    r.turn.as_str() == turn && r.owner == **id && !r.ended && r.document == path
                })
                .map(|(_, r)| r.clone())
                .ok_or_else(|| error("No active owner for this turn/document"))?;
            Run {
                agent: agent.clone(),
                model: model.clone(),
                plan: None,
                ended: false,
                prepared: false,
                ..primary
            }
        } else {
            Run {
                document: path.clone(),
                turn: TurnId::new(self.docs.ulid()).expect("ULID"),
                session: resumed
                    .as_ref()
                    .map(|m| m.id.clone())
                    .unwrap_or_else(|| SessionId::new(self.docs.ulid()).expect("ULID")),
                agent: agent.clone(),
                person: Actor::Person {
                    device: DeviceId::new(self.docs.ulid()).expect("id"),
                },
                model: model.clone(),
                owner: conn,
                plan: None,
                ended: false,
                prepared: false,
            }
        };
        let ordinal = self
            .cli_runs
            .values()
            .filter(|r| r.turn == run.turn)
            .count();
        let hues: Value = serde_json::from_str(include_str!(
            "../../../../packages/ui-tokens/source/tokens.json"
        ))
        .map_err(error)?;
        let names: Vec<String> = hues["tokens"]
            .as_array()
            .ok_or_else(|| error("Agent hue tokens are unavailable"))?
            .iter()
            .filter(|t| t["group"] == "agent")
            .filter_map(|t| {
                t["name"]
                    .as_str()
                    .and_then(|name| name.strip_prefix("agent/"))
            })
            .map(|hue| format!("{}{}", hue[..1].to_ascii_uppercase(), &hue[1..]))
            .collect();
        let name = match engine {
            "builtin" => names
                .get(ordinal)
                .ok_or_else(|| error("No unused agent hue for this lane"))?
                .as_str(),
            "claude-code" => "Claude Code",
            _ => "Codex",
        };
        if run.owner == conn {
            let log = self
                .log
                .as_ref()
                .ok_or_else(|| error("Session recording unavailable"))?;
            let doc = log
                .document_key(std::path::Path::new(&path))
                .map_err(error)?;
            if resumed.as_ref().is_some_and(|meta| meta.events == 0) {
                log.store().rename(&run.session, &text(p, "prompt")?.chars().take(120).collect::<String>()).map_err(error)?;
            }
            if resumed.is_none() {
                log.store()
                    .create(NewSession {
                        id: run.session.clone(),
                        doc,
                        kind: SessionKind::Engine {
                            engine: engine.into(),
                        },
                        agent: agent.clone(),
                        title: text(p, "prompt")?.chars().take(120).collect(),
                        created_at: self.docs.now(),
                    })
                    .map_err(error)?;
            }
            log.store()
                .append(
                    &run.session,
                    NewEvent {
                        at: self.docs.now(),
                        origin: run.person(),
                        body: EventBody::Prompt {
                            text: text(p, "prompt")?.into(),
                            context: vec![],
                            images: vec![],
                        },
                    },
                )
                .map_err(error)?;
            record_transition(
                log.store(),
                &run.person(),
                TurnInput::Start,
                self.docs.now(),
            )
            .map_err(error)?;
            record_transition(
                log.store(),
                &run.origin(),
                TurnInput::NoPlanNeeded,
                self.docs.now(),
            )
            .map_err(error)?;
        }
        let mut session = Session::for_agent(name, agent.clone(), run.session.clone());
        session.cwd = cwd;
        session.set_turn(Some(run.turn.clone()));
        session.log_path_as(path.clone(), run.session.clone());
        self.sessions.insert(conn, session);
        self.docs.name_agent(agent.clone(), name.into());
        let log = self
            .log
            .as_ref()
            .ok_or_else(|| error("Sessions unavailable"))?;
        let events = log.store().events(&run.session, None).map_err(error)?;
        let history = vellum_core::sessions::history(log.store(), &events).map_err(error)?;
        let result = json!({"turn":run.turn.as_str(),"agent":agent.as_str(),"session":run.session.as_str(),"document":path,"history":crate::cli::prompt::runner::encode_messages(history)});
        self.cli_runs.insert(conn, run);
        Ok(result)
    }
    fn cli_prepare(&mut self, conn: u64, p: &Value) -> Result<Value> {
        let run = self.cli_runs[&conn].clone();
        if run.owner != conn {
            return Err(error("Only the turn owner may prepare lanes"));
        }
        if run.prepared {
            return Err(error("Lanes are already prepared for this turn"));
        }
        let mode = text(p, "mode")?;
        if !matches!(
            mode,
            "split" | "side-by-side" | "explore-layout" | "explore-style"
        ) {
            return Err(error("Unknown parallel mode"));
        }
        let brief = text(p, "prompt")?;
        let agents = p["agents"]
            .as_array()
            .ok_or_else(|| error("Missing lane agents"))?;
        parallel::check_lanes(agents.len()).map_err(error)?;
        let agents: Vec<AgentId> = agents
            .iter()
            .map(|v| {
                self.cli_runs
                    .values()
                    .find(|r| {
                        r.turn == run.turn
                            && r.document == run.document
                            && !r.ended
                            && v.as_str() == Some(r.agent.as_str())
                    })
                    .map(|r| r.agent.clone())
                    .ok_or_else(|| error("Unknown lane agent"))
            })
            .collect::<Result<_>>()?;
        let unique: std::collections::BTreeSet<_> = agents.iter().collect();
        if unique.len() != agents.len() {
            return Err(error("Duplicate lane agent"));
        }
        let doc = self
            .docs
            .target(&run.document)
            .and_then(|t| t.snapshot())
            .map_err(error)?;
        let mut placeholders = vec![];
        let mut taken = vec![];
        let mut prompts = vec![];
        let plan = if mode == "split" {
            Some(
                run.plan
                    .as_ref()
                    .ok_or_else(|| error("Split requires a proposed plan"))?,
            )
        } else {
            None
        };
        let shares = plan
            .map(|plan| {
                parallel::split(
                    &plan.items.iter().map(|i| i.id).collect::<Vec<_>>(),
                    agents.len(),
                )
            })
            .unwrap_or_default();
        if let Some(plan) = plan {
            for item in plan.items.iter().filter(|i| i.is_new()) {
                let lane = shares
                    .iter()
                    .position(|s| s.contains(&item.id))
                    .expect("share");
                let (bounds, _) = crate::tools::free_space(
                    &mut self.docs,
                    &run.document,
                    &doc,
                    item.page.as_str(),
                    (item.width.unwrap_or(0.0), item.height.unwrap_or(0.0)),
                    None,
                    &taken,
                )
                .map_err(error)?;
                taken.push(bounds);
                placeholders.push(Placeholder {
                    item: item.id,
                    frame: NodeId::new(self.docs.ulid()).expect("id"),
                    name: item.name.clone(),
                    page: item.page.clone(),
                    bounds,
                    agent: agents[lane].clone(),
                    turn: run.turn.clone(),
                });
            }
            for share in &shares {
                prompts.push(parallel::split_prompt(
                    plan,
                    share,
                    &placeholders,
                    shares.len(),
                ));
            }
            self.cli_append(
                &run,
                EventBody::Plan(vellum_core::plans::PlanEvent::Built {
                    plan: plan.clone(),
                    placeholders: placeholders.clone(),
                }),
            )?;
        } else {
            let width = p["width"]
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0)
                .ok_or_else(|| error("Invalid width"))?;
            let height = p["height"]
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0 && *n <= 100_000.0)
                .ok_or_else(|| error("Invalid height"))?;
            let page = doc
                .pages
                .first()
                .ok_or_else(|| error("No page for options"))?;
            for (n, agent) in agents.iter().enumerate() {
                let (bounds, _) = crate::tools::free_space(
                    &mut self.docs,
                    &run.document,
                    &doc,
                    page.id.as_str(),
                    (width, height),
                    None,
                    &taken,
                )
                .map_err(error)?;
                taken.push(bounds);
                let placeholder = Placeholder {
                    item: n as u32 + 1,
                    frame: NodeId::new(self.docs.ulid()).expect("id"),
                    name: parallel::option_label(n as u32 + 1),
                    page: page.id.clone(),
                    bounds,
                    agent: agent.clone(),
                    turn: run.turn.clone(),
                };
                prompts.push(if mode == "side-by-side" {
                    parallel::side_by_side_prompt(brief, n as u32 + 1, agents.len(), &placeholder)
                } else if n == 0 {
                    parallel::explore_first_prompt(brief, agents.len(), &placeholder)
                } else {
                    parallel::explore_prompt(
                        brief,
                        if mode == "explore-layout" {
                            parallel::Axis::Layout
                        } else {
                            parallel::Axis::Style
                        },
                        n + 1,
                        agents.len(),
                        &placeholders[0].frame,
                        &placeholder,
                    )
                });
                placeholders.push(placeholder);
            }
        }
        self.docs.reserve(&run.document, placeholders);
        self.cli_runs.get_mut(&conn).expect("run").prepared = true;
        let mode = match mode {
            "split" => Mode::Split,
            "side-by-side" => Mode::SideBySide,
            "explore-layout" => Mode::Explore(parallel::Axis::Layout),
            "explore-style" => Mode::Explore(parallel::Axis::Style),
            _ => return Err(error("Unknown parallel mode")),
        };
        self.cli_append(
            &run,
            EventBody::Parallel(ParallelEvent::Lanes {
                mode,
                lanes: agents
                    .iter()
                    .enumerate()
                    .map(|(n, agent)| LaneRecord {
                        agent: agent.clone(),
                        name: self.docs.agents().get(agent).cloned().unwrap_or_default(),
                        model: run.model.clone(),
                        items: shares.get(n).cloned().unwrap_or_default(),
                        task: None,
                    })
                    .collect(),
            }),
        )?;
        Ok(json!({"prompts":prompts}))
    }
    pub(super) fn cli_fill(&mut self, conn: u64) {
        let Some(run) = self.cli_runs.get(&conn).cloned() else {
            return;
        };
        let Ok(doc) = self.docs.target(&run.document).and_then(|t| t.snapshot()) else {
            return;
        };
        let frames: Vec<_> = self
            .docs
            .placeholders(&run.document)
            .iter()
            .filter(|p| {
                p.placeholder.turn == run.turn
                    && p.placeholder.agent == run.agent
                    && !p.filling
                    && vellum_core::ops::find(&doc, &p.placeholder.frame).is_some()
            })
            .map(|p| p.placeholder.frame.clone())
            .collect();
        for frame in frames {
            if self
                .docs
                .target(&run.document)
                .and_then(|t| t.lease(&frame, &run.agent, LeaseChange::Claim))
                .is_ok()
            {
                self.docs.filling(&run.document, &frame);
            }
        }
    }
    fn cli_finish(&mut self, conn: u64, failed: bool) -> Result<()> {
        let run = self
            .cli_runs
            .get(&conn)
            .filter(|r| !r.ended)
            .cloned()
            .ok_or_else(|| error("No active CLI turn"))?;
        let lanes: Vec<_> = self
            .cli_runs
            .iter()
            .filter(|(id, r)| {
                !r.ended && (**id == conn || (run.owner == conn && r.turn == run.turn))
            })
            .map(|(id, r)| (*id, r.clone()))
            .collect();
        // Guard changes must succeed before optional/damaged recording can interfere with teardown.
        for (id, lane) in &lanes {
            self.docs
                .control_all(&AgentControl::Stop(lane.agent.clone()));
            self.cli_runs.get_mut(id).expect("lane").ended = true;
        }
        self.docs.unreserve(&run.document, |p| {
            p.turn == run.turn && (run.owner == conn || p.agent == run.agent)
        });
        for (id, lane) in lanes {
            self.cli_append(
                &lane,
                EventBody::Parallel(ParallelEvent::LaneEnded {
                    agent: lane.agent.clone(),
                    end: if failed || id != conn {
                        LaneEnd::Failed
                    } else {
                        LaneEnd::Done
                    },
                }),
            )?;
        }
        if run.owner == conn {
            if run.plan.is_some() {
                self.cli_append(
                    &run,
                    EventBody::Plan(if failed {
                        vellum_core::plans::PlanEvent::Abandoned
                    } else {
                        vellum_core::plans::PlanEvent::Finished
                    }),
                )?;
            }
            self.log
                .as_ref()
                .ok_or_else(|| error("Session recording unavailable"))?
                .record_turn_event(
                    &run.origin(),
                    &run.model,
                    &TurnEvent::Ended(if failed {
                        TurnEnd::Failed(EngineError::Unavailable)
                    } else {
                        TurnEnd::Done
                    }),
                    self.docs.now(),
                )
                .map_err(error)?;
        }
        Ok(())
    }
    pub(super) fn cli_disconnected(&mut self, conn: u64) {
        if self.cli_runs.get(&conn).is_some_and(|r| !r.ended) {
            let _ = self.cli_finish(conn, true);
        }
        self.cli_runs.remove(&conn);
    }
}

pub(crate) fn decode_message(value: &Value) -> Result<ChatMessage> {
    let role = match text(value, "role")? {
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        _ => return Err(error("Invalid message role")),
    };
    let content = value["content"]
        .as_array()
        .ok_or_else(|| error("Missing content"))?
        .iter()
        .map(|part| {
            Ok(match text(part, "type")? {
                "text" => ChatPart::Text(text(part, "text")?.into()),
                "image" => ChatPart::Image {
                    mime: text(part, "mime")?.into(),
                    base64: text(part, "base64")?.into(),
                },
                "call" => ChatPart::ToolCall(ModelCall {
                    id: text(part, "id")?.into(),
                    name: text(part, "name")?.into(),
                    arguments: part["arguments"].clone(),
                    metadata: part["metadata"].clone(),
                }),
                "result" => ChatPart::ToolResult {
                    id: text(part, "id")?.into(),
                    name: text(part, "name")?.into(),
                    result: part["result"].clone(),
                    failed: part["failed"] == true,
                },
                "continuation" => ChatPart::Continuation(part["value"].clone()),
                _ => return Err(error("Invalid message part")),
            })
        })
        .collect::<Result<_>>()?;
    Ok(ChatMessage { role, content })
}
