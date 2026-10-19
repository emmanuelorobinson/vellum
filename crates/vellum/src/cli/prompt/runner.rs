//! Stream polling and remote parallel lanes. Core owns splitting, reservations and lane instructions.
use super::*;

pub(super) struct RequestRun<'a> {
    pub dir: &'a RunDir,
    pub args: &'a PromptArgs,
    pub engine_id: &'a str,
    pub full_model: &'a str,
    pub engine: &'a dyn AgentEngine,
    pub request: TurnRequest,
    pub bridge: &'a RemoteBridge<'a>,
    pub measured: &'a RefCell<ModelUsage>,
}
pub(super) fn run(run: RequestRun<'_>) -> io::Result<TurnEnd> {
    let RequestRun {
        dir,
        args,
        engine_id,
        full_model,
        engine,
        request,
        bridge,
        measured,
    } = run;
    if args.parallel == 1 && args.mode == "split" {
        return block_on(drive(engine, request, bridge, measured));
    }
    if args.parallel < 2 {
        return Err(io::Error::other(
            "Side by side and Explore require --parallel 2 to 6",
        ));
    }
    if args.mode == "split" {
        bridge.planning.set(true);
        let result = block_on(drive(engine, request.clone(), bridge, measured));
        bridge.planning.set(false);
        if result? != TurnEnd::Done || !bridge.proposed.get() {
            return Err(io::Error::other(
                "Split planning failed; no plan was proposed",
            ));
        }
    }
    let mut lanes = Vec::new();
    let mut agents = Vec::new();
    for n in 0..args.parallel {
        let mut rpc = Rpc::connect(dir)?;
        rpc.open(&bridge.file, false)?;
        let joined=rpc.request("vellum/cli",json!({"action":"turn","document":bridge.file,"engine":engine_id,"prompt":request.prompt,"model":request.model,"join":request.turn.as_str()}))?;
        agents.push(joined["agent"].clone());
        if bridge.out.json {
            println!(
                "{}",
                json!({"event":"lane_start","task":bridge.task,"lane":n+1,"turn":joined["turn"],"agent":joined["agent"]})
            );
        }
        lanes.push(RemoteBridge::new(
            rpc,
            bridge.file.clone(),
            bridge.out,
            args,
            bridge.task,
            bridge.cancellation.clone(),
        ));
    }
    let prepared=bridge.rpc.borrow_mut().request("vellum/cli",json!({"action":"prepare","mode":args.mode,"prompt":request.prompt,"agents":agents,"width":args.width,"height":args.height}))?;
    let prompts = prepared["prompts"]
        .as_array()
        .ok_or_else(|| io::Error::other("Missing lane prompts"))?;
    let engines: Vec<_> = (0..prompts.len())
        .map(|_| make_engine(dir, args, engine_id, full_model).map(|(e, _)| e))
        .collect::<io::Result<_>>()?;
    let requests: Vec<_> = prompts
        .iter()
        .map(|p| {
            let mut r = request.clone();
            r.prompt = p.as_str().unwrap_or_default().into();
            r
        })
        .collect();
    let mut ends: Vec<io::Result<TurnEnd>> = vec![];
    if args.mode.starts_with("explore") {
        for ((engine, request), lane) in engines.iter().zip(requests).zip(&lanes) {
            let end = block_on(drive(&**engine, request, lane, measured));
            let done = matches!(end, Ok(TurnEnd::Done));
            finish_lane(lane, !done)?;
            ends.push(end);
            if !done {
                break;
            }
        }
    } else {
        let mut futures: Vec<_> = engines
            .iter()
            .zip(requests)
            .zip(&lanes)
            .map(|((e, r), b)| Some(Box::pin(drive(&**e, r, b, measured))))
            .collect();
        block_on(std::future::poll_fn(|cx| {
            let mut pending = false;
            for (n, slot) in futures.iter_mut().enumerate() {
                if let Some(future) = slot {
                    match future.as_mut().poll(cx) {
                        Poll::Ready(end) => {
                            let done = matches!(end, Ok(TurnEnd::Done));
                            ends.push(end);
                            ends.push(finish_lane(&lanes[n], !done).map(|()| TurnEnd::Done));
                            *slot = None;
                        }
                        Poll::Pending => pending = true,
                    }
                }
            }
            if pending {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }));
    }
    let mut success = !ends.is_empty();
    for end in ends {
        success &= end? == TurnEnd::Done;
    }
    Ok(if success {
        TurnEnd::Done
    } else {
        TurnEnd::Failed(EngineError::Unavailable)
    })
}
fn finish_lane(lane: &RemoteBridge<'_>, failed: bool) -> io::Result<()> {
    lane.rpc
        .borrow_mut()
        .request("vellum/cli", json!({"action":"end","failed":failed}))
        .map(|_| ())
}

pub(super) async fn drive(
    engine: &dyn AgentEngine,
    request: TurnRequest,
    bridge: &RemoteBridge<'_>,
    measured: &RefCell<ModelUsage>,
) -> io::Result<TurnEnd> {
    let monitor = Monitor::start(bridge)?;
    if bridge.cancellation.is_cancelled() || bridge.native_stop.is_cancelled() {
        return Ok(if monitor.failed() {
            TurnEnd::Failed(EngineError::Unavailable)
        } else {
            TurnEnd::Stopped
        });
    }
    let turn = request.turn.clone();
    let max_failed = request.limits.max_failed_calls;
    let mut failures = 0u32;
    let mut stream = engine.run_turn(request, bridge).map_err(io::Error::other)?;
    let mut end = None;
    loop {
        let next = stream.next();
        let mut next = std::pin::pin!(next);
        let event = std::future::poll_fn(|cx| {
            if bridge.cancellation.poll_cancelled(cx).is_ready()
                || bridge.native_stop.poll_cancelled(cx).is_ready()
            {
                return Poll::Ready(None);
            }
            next.as_mut().poll(cx)
        })
        .await;
        if bridge.cancellation.is_cancelled() || bridge.native_stop.is_cancelled() {
            engine.interrupt(&turn);
            end = Some(if monitor.failed() && !bridge.cancellation.is_cancelled() {
                TurnEnd::Failed(EngineError::Unavailable)
            } else {
                TurnEnd::Stopped
            });
            break;
        }
        let Some(event) = event else {
            break;
        };
        if bridge.planning.get() && bridge.proposed.get() {
            engine.interrupt(&turn);
            end = Some(TurnEnd::Done);
            break;
        }
        if bridge.preview_failed.get() {
            engine.interrupt(&turn);
            end = Some(TurnEnd::Failed(EngineError::Unavailable));
            break;
        }
        if matches!(&event, TurnEvent::ToolFinished { failed: true, .. }) {
            failures = failures.saturating_add(1);
            if failures > max_failed {
                engine.interrupt(&turn);
                end = Some(TurnEnd::Failed(EngineError::TooManyFailedCalls));
                break;
            }
        }
        match event {
            TurnEvent::Text(text) => {
                if bridge.out.json {
                    println!("{}", json!({"event":"text","task":bridge.task,"text":text}));
                } else {
                    print!("{text}");
                    io::stdout().flush()?;
                }
            }
            TurnEvent::Reasoning(text) if bridge.verbose => {
                if bridge.out.json {
                    println!(
                        "{}",
                        json!({"event":"reasoning","task":bridge.task,"text":text})
                    );
                } else {
                    eprintln!("{text}");
                }
            }
            TurnEvent::Usage(part) => {
                merge_usage(&mut measured.borrow_mut(), part);
                bridge.rpc.borrow_mut().request(
                    "vellum/cli",
                    json!({"action":"usage","usage":usage_json(part)}),
                )?;
            }
            TurnEvent::Messages(messages) => {
                bridge.rpc.borrow_mut().request(
                    "vellum/cli",
                    json!({"action":"messages","messages":encode_messages(messages)}),
                )?;
            }
            TurnEvent::Ended(result) => end = Some(result),
            _ => {}
        }
    }
    if !bridge.out.json {
        println!();
    }
    if bridge.preview_failed.get() {
        return Ok(TurnEnd::Failed(EngineError::Unavailable));
    }
    end.ok_or_else(|| io::Error::other("Engine disconnected without a terminal event"))
}
fn encode_messages(messages: Vec<ChatMessage>) -> Value {
    json!(messages.into_iter().map(|message|json!({"role":if message.role==ChatRole::User{"user"}else{"assistant"},"content":message.content.into_iter().map(|part|match part{
        ChatPart::Text(text)=>json!({"type":"text","text":text}),
        ChatPart::Image{mime,base64}=>json!({"type":"image","mime":mime,"base64":base64}),
        ChatPart::ToolCall(call)=>json!({"type":"call","id":call.id,"name":call.name,"arguments":call.arguments,"metadata":call.metadata}),
        ChatPart::ToolResult{id,name,result,failed}=>json!({"type":"result","id":id,"name":name,"result":result,"failed":failed}),
        ChatPart::Continuation(value)=>json!({"type":"continuation","value":value}),
    }).collect::<Vec<_>>()})).collect::<Vec<_>>())
}

/// Native polling keeps Pause/Stop responsive even while an engine has no events to yield.
/// The worker has no document store and reads only scoped control metadata.
struct Monitor {
    closed: Arc<std::sync::atomic::AtomicBool>,
    failed: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Monitor {
    fn failed(&self) -> bool {
        self.failed.load(std::sync::atomic::Ordering::Acquire)
    }
    fn start(bridge: &RemoteBridge<'_>) -> io::Result<Self> {
        use std::sync::atomic::{AtomicBool, Ordering};
        let mut primary = bridge.rpc.borrow_mut();
        let dir = primary.run_dir.clone();
        let params = json!({"action":"control","document":bridge.file,"turn":primary.lane["turn"],"agent":primary.lane["agent"]});
        let state = primary.request("vellum/cli", params.clone())?;
        let local = bridge.native_stop.clone();
        let all = bridge.cancellation.clone();
        if state["all"] == true {
            all.cancel();
        } else if state["stop"] == true {
            local.cancel();
        }
        drop(primary);
        let closed = Arc::new(AtomicBool::new(false));
        let closing = closed.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let worker_failed = failed.clone();
        let worker = std::thread::Builder::new()
            .name("vellum-cli-control".into())
            .spawn(move || {
                let mut rpc = match Rpc::connect(&dir) {
                    Ok(rpc) => rpc,
                    Err(_) => {
                        worker_failed.store(true, Ordering::Release);
                        local.cancel();
                        return;
                    }
                };
                while !closing.load(Ordering::Acquire) {
                    match rpc.request("vellum/cli", params.clone()) {
                        Ok(state) if state["all"] == true => {
                            all.cancel();
                            break;
                        }
                        Ok(state) if state["stop"] == true => {
                            local.cancel();
                            break;
                        }
                        Err(_) => {
                            worker_failed.store(true, Ordering::Release);
                            local.cancel();
                            break;
                        }
                        _ => {}
                    }
                    std::thread::park_timeout(std::time::Duration::from_millis(200));
                }
            })?;
        Ok(Self {
            closed,
            failed,
            worker: Some(worker),
        })
    }
}
impl Drop for Monitor {
    fn drop(&mut self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
