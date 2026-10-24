//! Agent mode over the existing engines and an authenticated remote tool bridge.
pub(crate) mod runner;

use super::commands::{ExportArgs, Output, Rpc, export};
use clap::Args;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use vellum_core::ports::*;
use vellum_ipc_local::RunDir;

#[derive(Args, Clone)]
pub struct PromptArgs {
    /// Continue an engine session of this document (desktop host).
    #[arg(long)]
    pub session: Option<String>,
    /// FILE PROMPT, or PROMPT with --out FILE.
    #[arg(num_args=0..=2)]
    pub words: Vec<String>,
    #[arg(long, value_parser=["builtin","claude-code","codex"], default_value="builtin")]
    pub engine: String,
    /// Builtin: provider/model; installed engines: their model ID.
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, value_parser=["low","medium","high"])]
    pub effort: Option<String>,
    #[arg(long)]
    pub attach: Vec<PathBuf>,
    #[arg(long)]
    pub repo: Option<PathBuf>,
    #[arg(long, default_value_t=1, value_parser=clap::value_parser!(u8).range(1..=6))]
    pub parallel: u8,
    #[arg(long, default_value="split",value_parser=["split","side-by-side","explore-layout","explore-style"])]
    pub mode: String,
    /// Write a JSON usage report even if a task fails.
    #[arg(long)]
    pub usage: Option<PathBuf>,
    /// Print a screenshot artifact after each canvas change.
    #[arg(long)]
    pub preview_each_change: bool,
    #[arg(long, default_value_t = 5)]
    pub max_failed_calls: u32,
    #[arg(long = "export")]
    pub export_file: Option<PathBuf>,
    #[arg(long,default_value_t=1,value_parser=clap::value_parser!(u8).range(1..=3))]
    pub export_scale: u8,
    /// Include reasoning and complete tool arguments/results.
    #[arg(long)]
    pub verbose: bool,
    /// A JSON array of {document,prompt,out?,attach?,engine?,model?,effort?,export?}; every task runs.
    #[arg(long, conflicts_with = "words")]
    pub tasks: Option<PathBuf>,
    #[arg(long)]
    pub out: Option<String>,
    /// Explicitly declare that the selected builtin model supports images.
    #[arg(long)]
    pub vision: bool,
    /// Model context size; omit when unknown.
    #[arg(long)]
    pub context_window: Option<u64>,
    /// Frame width for Side by side/Explore.
    #[arg(long, default_value_t = 1440.0)]
    pub width: f64,
    /// Frame height for Side by side/Explore.
    #[arg(long, default_value_t = 900.0)]
    pub height: f64,
}
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Task {
    #[serde(alias = "file")]
    document: Option<String>,
    prompt: String,
    out: Option<String>,
    #[serde(default)]
    attach: Vec<PathBuf>,
    engine: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    #[serde(rename = "export")]
    export_file: Option<PathBuf>,
}

pub(super) fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

pub fn run(dir: &RunDir, out: &Output, args: PromptArgs) -> io::Result<bool> {
    if let Some(repo) = &args.repo {
        std::env::set_current_dir(repo)?;
    }
    let tasks: Vec<io::Result<Task>> = if let Some(file) = &args.tasks {
        let tasks: Vec<Value> =
            serde_json::from_slice(&std::fs::read(file)?).map_err(io::Error::other)?;
        if tasks.is_empty() {
            return Err(io::Error::other("Task file is empty"));
        }
        tasks
            .into_iter()
            .map(|task| serde_json::from_value(task).map_err(io::Error::other))
            .collect()
    } else {
        let (document, prompt) = match (args.words.as_slice(), &args.out) {
            ([file, prompt], None) => (Some(file.clone()), prompt.clone()),
            ([prompt], Some(_)) => (None, prompt.clone()),
            _ => {
                return Err(io::Error::other(
                    "Use prompt FILE PROMPT, prompt --out FILE PROMPT, or prompt --tasks FILE",
                ));
            }
        };
        vec![Ok(Task {
            document,
            prompt,
            out: args.out.clone(),
            attach: args.attach.clone(),
            engine: None,
            model: None,
            effort: None,
            export_file: args.export_file.clone(),
        })]
    };
    let cancellation = Cancellation::default();
    let notified = cancellation.clone();
    let _interrupt = vellum_ipc_local::on_interrupt(move || notified.cancel())?;
    let mut rows = vec![];
    let mut success = true;
    for (index, task) in tasks.into_iter().enumerate() {
        let measured = RefCell::new(ModelUsage::default());
        let result =
            task.and_then(|task| one(dir, out, &args, task, index + 1, &measured, &cancellation));
        let (ok, usage, error) = match result {
            Ok(value) => (true, value, None),
            Err(error) => (
                false,
                usage_json(*measured.borrow()),
                Some(error.to_string()),
            ),
        };
        success &= ok;
        let row = json!({"event":"task_end","task":index+1,"ok":ok,"usage":usage,"error":error});
        if out.json {
            println!("{row}");
        } else if let Some(message) = &error {
            out.error("prompt", message);
        } else {
            let count = |key: &str| {
                usage[key]
                    .as_u64()
                    .map_or_else(|| "unavailable".into(), |n| n.to_string())
            };
            let cost = usage["estimated_micro_usd"].as_u64().map_or_else(
                || "Estimated cost unavailable".into(),
                |n| format!("Estimated ${:.6}", n as f64 / 1_000_000.0),
            );
            println!(
                "✓ Turn complete · {} input · {} output · {cost}",
                count("input_tokens"),
                count("output_tokens")
            );
        }
        rows.push(row);
    }
    if let Some(path) = &args.usage {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"ok":success,"tasks":rows}))
                .map_err(io::Error::other)?,
        )?;
    }
    Ok(success)
}

fn one(
    dir: &RunDir,
    out: &Output,
    args: &PromptArgs,
    task: Task,
    number: usize,
    measured: &RefCell<ModelUsage>,
    cancellation: &Cancellation,
) -> io::Result<Value> {
    if cancellation.is_cancelled() {
        return Err(io::Error::other(
            "Turn stopped; remaining tasks were not started",
        ));
    }
    if task.prompt.trim().is_empty() {
        return Err(io::Error::other("A task needs a nonempty prompt"));
    }
    let engine_id = task.engine.as_deref().unwrap_or(&args.engine);
    if engine_id != "builtin" && task.model.is_none() && args.model.is_none() {
        return Err(io::Error::other(
            "Choose --model for the installed engine; vellum models describes the catalog boundary",
        ));
    }
    let model = task
        .model
        .as_ref()
        .or(args.model.as_ref())
        .cloned()
        .unwrap_or_else(|| {
            if engine_id == "builtin" {
                "openrouter/deepseek/deepseek-v4.1-flash".into()
            } else {
                "default".into()
            }
        });
    let effort = task
        .effort
        .as_ref()
        .or(args.effort.as_ref())
        .map(|s| match s.as_str() {
            "low" => Ok(ReasoningEffort::Low),
            "medium" => Ok(ReasoningEffort::Medium),
            "high" => Ok(ReasoningEffort::High),
            _ => Err(io::Error::other("Effort must be low, medium or high")),
        })
        .transpose()?;
    let full_model = model.clone();
    let mut configured = args.clone();
    configured.effort = task.effort.clone().or(args.effort.clone());
    let args = &configured;
    let (engine, model) = make_engine(dir, args, engine_id, &model)?;
    // Validate every attachment before creating an output or starting an engine.
    let mut text = task.prompt;
    let mut images = vec![];
    for file in &task.attach {
        let bytes = std::fs::read(file)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(io::Error::other("Attachment exceeds 8 MiB"));
        }
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some("image/png")
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            Some("image/jpeg")
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            Some("image/webp")
        } else {
            None
        };
        if let Some(mime) = mime {
            if engine_id == "builtin" && !args.vision {
                return Err(io::Error::other(
                    "Image attachment needs --vision to declare model support",
                ));
            }
            images.push((mime.into(), vellum_core::base64::encode(&bytes)));
        } else {
            text.push_str(&format!(
                "\n\nAttachment {}:\n{}",
                file.display(),
                String::from_utf8(bytes).map_err(|_| io::Error::other(
                    "Attachment must be UTF-8 text, PNG, JPEG or WEBP"
                ))?
            ));
        }
    }
    let mut rpc = Rpc::connect(dir)?;
    let file = match (&task.document, &task.out) {
        (Some(file), None) => {
            rpc.open(file, false)?;
            file.clone()
        }
        (None, Some(file)) => {
            rpc.request("vellum/cli", json!({"action":"new","document":file}))?;
            rpc.open(file, false)?;
            file.clone()
        }
        _ => return Err(io::Error::other("Each task names document or out")),
    };
    let turn = rpc.request(
        "vellum/cli",
        json!({"action":"turn","document":file,"engine":engine_id,"prompt":text,"model":model,"session":args.session}),
    )?;
    if out.json {
        println!(
            "{}",
            json!({"event":"turn_started","session":turn["session"],"turn":turn["turn"]})
        );
    }
    let id = vellum_types::TurnId::new(
        turn["turn"]
            .as_str()
            .ok_or_else(|| io::Error::other("Missing turn ID"))?,
    )
    .map_err(io::Error::other)?;
    let mut request = TurnRequest::new(id, model.clone(), text);
    request.history = turn["history"]
        .as_array()
        .into_iter()
        .flatten()
        .map(crate::mcp::cli::decode_message)
        .collect::<Result<_, _>>()
        .map_err(|(_, message)| io::Error::other(message))?;
    request.images = images;
    request.effort = effort;
    request.limits.max_failed_calls = args.max_failed_calls;
    request.skills = SkillSource::list(&crate::content::BuiltinSkills).map_err(io::Error::other)?;
    let bridge = RemoteBridge::new(rpc, file.clone(), out, args, number, cancellation.clone());
    let end = runner::run(runner::RequestRun {
        dir,
        args,
        engine_id,
        full_model: &full_model,
        engine: &*engine,
        request,
        bridge: &bridge,
        measured,
    })?;
    let end = if cancellation.is_cancelled() {
        TurnEnd::Stopped
    } else {
        end
    };
    bridge.rpc.borrow_mut().save(&file)?;
    bridge.rpc.borrow_mut().request(
        "vellum/cli",
        json!({"action":if end==TurnEnd::Stopped{"stop"}else{"end"},"failed":end!=TurnEnd::Done}),
    )?;
    match end {
        TurnEnd::Done => {}
        TurnEnd::Stopped => {
            return Err(io::Error::other(
                "Turn stopped; completed changes were kept",
            ));
        }
        TurnEnd::Failed(error) => return Err(io::Error::other(error)),
    }
    if let Some(path) = task.export_file.or(args.export_file.clone()) {
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some("jpg" | "jpeg") => "jpeg",
            Some("html") => "html-css",
            Some("png") => "png",
            Some("webp") => "webp",
            Some("svg") => "svg",
            Some("pdf") => "pdf",
            _ => {
                return Err(io::Error::other(
                    "Export filename needs png, jpg, webp, svg, pdf or html extension",
                ));
            }
        };
        export(
            &mut bridge.rpc.borrow_mut(),
            &ExportArgs {
                file,
                frame: vec![],
                format: format.into(),
                scale: args.export_scale,
                scaffold: true,
                output: path,
            },
        )?;
    }
    let mut usage = usage_json(*measured.borrow());
    let record = vellum_core::sessions::usage_record(
        Some(&crate::sessions::pricing()),
        &model,
        &measured.borrow(),
    );
    if measured.borrow().input_tokens.is_some() && measured.borrow().output_tokens.is_some() {
        usage["estimated_micro_usd"] = json!(record.estimate);
    }
    Ok(usage)
}
fn usage_json(usage: ModelUsage) -> Value {
    json!({"input_tokens":usage.input_tokens,"output_tokens":usage.output_tokens,"reasoning_tokens":usage.reasoning_tokens,"cached_input_tokens":usage.cached_input_tokens,"cache_write_tokens":usage.cache_write_tokens,"estimated_micro_usd":null})
}

fn merge_usage(total: &mut ModelUsage, part: ModelUsage) {
    fn add(a: &mut Option<u64>, b: Option<u64>) {
        *a = match (*a, b) {
            (Some(a), Some(b)) => Some(a.saturating_add(b)),
            (None, Some(b)) => Some(b),
            (a, None) => a,
        };
    }
    add(&mut total.input_tokens, part.input_tokens);
    add(&mut total.output_tokens, part.output_tokens);
    add(&mut total.reasoning_tokens, part.reasoning_tokens);
    add(&mut total.cached_input_tokens, part.cached_input_tokens);
    add(&mut total.cache_write_tokens, part.cache_write_tokens);
}

struct RemoteBridge<'a> {
    rpc: RefCell<Rpc>,
    file: String,
    out: &'a Output,
    verbose: bool,
    preview: bool,
    task: usize,
    planning: Cell<bool>,
    proposed: Cell<bool>,
    preview_failed: Cell<bool>,
    native_stop: Cancellation,
    cancellation: Cancellation,
}
impl<'a> RemoteBridge<'a> {
    fn new(
        rpc: Rpc,
        file: String,
        out: &'a Output,
        args: &PromptArgs,
        task: usize,
        cancellation: Cancellation,
    ) -> Self {
        Self {
            rpc: RefCell::new(rpc),
            file,
            out,
            verbose: args.verbose,
            preview: args.preview_each_change,
            task,
            planning: Cell::new(false),
            proposed: Cell::new(false),
            preview_failed: Cell::new(false),
            native_stop: Cancellation::default(),
            cancellation,
        }
    }
}
impl ToolBridge for RemoteBridge<'_> {
    fn tools(&self) -> Vec<BridgeTool> {
        let mut tools: Vec<_> = crate::tools::Registry::standard()
            .tools()
            .filter(|t| !self.planning.get() || t.read_only())
            .map(|t| BridgeTool {
                tool: ModelTool {
                    name: t.name().into(),
                    description: t.description().into(),
                    parameters: serde_json::to_value(t.input_schema()).expect("schema"),
                },
                read_only: t.read_only(),
            })
            .collect();
        if self.planning.get() {
            tools.push(BridgeTool {
                tool: ModelTool {
                    name: "propose_plan".into(),
                    description: crate::orchestrate::PROPOSE_DESCRIPTION.into(),
                    parameters: serde_json::to_value(schemars::schema_for!(
                        crate::orchestrate::ProposeInput
                    ))
                    .expect("schema"),
                },
                read_only: false,
            });
        }
        tools
    }
    fn call(&self, name: &str, mut input: Value) -> ToolReply {
        if self.cancellation.is_cancelled() || self.native_stop.is_cancelled() {
            return ToolReply {
                failed: true,
                value: json!({"error":{"message":"Turn stopped"}}),
            };
        }
        if self.planning.get() {
            if name == "propose_plan" {
                return match self
                    .rpc
                    .borrow_mut()
                    .request("vellum/cli", json!({"action":"plan","plan":input}))
                {
                    Ok(value) => {
                        self.proposed.set(true);
                        ToolReply {
                            value,
                            failed: false,
                        }
                    }
                    Err(error) => ToolReply {
                        value: json!({"error":{"message":error.to_string()}}),
                        failed: true,
                    },
                };
            }
            if self.proposed.get()
                || !crate::tools::Registry::standard()
                    .get(name)
                    .is_some_and(|t| t.read_only())
            {
                return ToolReply {
                    value: json!({"error":{"message":"Propose a plan, then end the planning run before building"}}),
                    failed: true,
                };
            }
        }
        if name != "open_document" && input.is_object() {
            if let Some(document) = input.get("document").and_then(Value::as_str)
                && document != self.file
            {
                return ToolReply {
                    failed: true,
                    value: json!({"error":{"message":"This turn is scoped to its document"}}),
                };
            }
            let registry = crate::tools::Registry::standard();
            if registry.get(name).is_some_and(|t| {
                serde_json::to_value(t.input_schema())
                    .ok()
                    .is_some_and(|s| s["properties"].get("document").is_some())
            }) {
                input["document"] = json!(self.file);
            }
        }
        if name == "open_document" && input["path"] != self.file {
            return ToolReply {
                failed: true,
                value: json!({"error":{"message":"This turn cannot open another document"}}),
            };
        }
        let mut rpc = self.rpc.borrow_mut();
        let result = rpc.tool(name, input.clone());
        let failed = result.is_err();
        let value = match result {
            Ok(value) => value,
            Err(error) => json!({"error":{"message":error.to_string()}}),
        };
        if self.out.json {
            println!(
                "{}",
                json!({"event":"step","task":self.task,"tool":name,"ok":!failed,"step":rpc.step,"arguments":self.verbose.then_some(&input),"result":self.verbose.then_some(&value)})
            );
        } else {
            println!("{}", step_line(name, failed, rpc.step.as_ref()));
            if self.verbose {
                println!("{input}\n{value}");
            }
        }
        let changed_target = rpc
            .step
            .as_ref()
            .and_then(|s| s["target"]["id"].as_str())
            .map(str::to_owned);
        if self.preview
            && !failed
            && crate::tools::Registry::standard()
                .get(name)
                .is_some_and(|t| !t.read_only())
        {
            let preview = (|| {
                let opened = rpc.open(&self.file, false)?;
                let node = changed_target
                    .as_deref()
                    .or_else(|| opened["pages"][0]["id"].as_str())
                    .ok_or_else(|| io::Error::other("Document has no page to preview"))?;
                rpc.tool("screenshot", json!({"document":self.file,"node":node}))
            })();
            match preview {
                Ok(value) => {
                    let saved = value["image"]["base64"]
                        .as_str()
                        .and_then(vellum_core::base64::decode)
                        .ok_or_else(|| io::Error::other("Preview returned invalid PNG bytes"))
                        .and_then(|bytes| rpc.preview_file(&bytes));
                    match saved {
                        Ok(path) => {
                            if self.out.json {
                                println!(
                                    "{}",
                                    json!({"event":"preview","task":self.task,"path":path})
                                );
                            } else {
                                println!("✓ Preview {}", path.display());
                            }
                        }
                        Err(error) => {
                            self.preview_failed.set(true);
                            return ToolReply {
                                failed: true,
                                value: json!({"error":{"message":format!("Change applied but preview could not be saved: {error}")}}),
                            };
                        }
                    }
                }
                Err(error) => {
                    self.preview_failed.set(true);
                    self.out.error(
                        "preview",
                        &format!("Change applied but preview failed: {error}"),
                    );
                    return ToolReply {
                        failed: true,
                        value: json!({"error":{"message":format!("Change applied but preview failed: {error}")}}),
                    };
                }
            }
        }
        ToolReply { value, failed }
    }
}

fn make_engine(
    dir: &RunDir,
    args: &PromptArgs,
    id: &str,
    model: &str,
) -> io::Result<(Box<dyn AgentEngine>, String)> {
    let repo = std::env::current_dir()?;
    let binary = std::env::current_exe()?;
    let private = dir.path().join("cli-turns");
    match id {
        "claude-code" => {
            return Ok((
                Box::new(crate::installed_engines::claude(
                    find_program("claude").ok_or_else(|| {
                        io::Error::other(
                            "Claude Code is not installed; install it or use --engine builtin",
                        )
                    })?,
                    repo,
                    binary,
                    private,
                )),
                model.into(),
            ));
        }
        "codex" => {
            return Ok((
                Box::new(crate::installed_engines::codex(
                    find_program("codex").ok_or_else(|| {
                        io::Error::other(
                            "Codex is not installed; install it or use --engine builtin",
                        )
                    })?,
                    repo,
                    binary,
                    private,
                )),
                model.into(),
            ));
        }
        "builtin" => {}
        _ => {
            return Err(io::Error::other(
                "Unknown engine; choose builtin, claude-code or codex",
            ));
        }
    }
    let (provider, model) = model
        .split_once('/')
        .ok_or_else(|| io::Error::other("Builtin --model must be provider/model"))?;
    if model.trim().is_empty() {
        return Err(io::Error::other("Model ID is empty"));
    }
    let endpoint=std::env::var("VELLUM_MODEL_ENDPOINT").ok().unwrap_or_else(||match provider {
        "openai"=>"https://api.openai.com/v1/responses".into(),"anthropic"=>"https://api.anthropic.com/v1/messages".into(),
        "google"=>format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"),
        other=>vellum_llm_openai_compat::presets().iter().find(|p|p.id==other).map(|p|p.endpoint.to_owned()).unwrap_or_default(),
    });
    if endpoint.is_empty() {
        return Err(io::Error::other(
            "Unknown provider; set VELLUM_MODEL_ENDPOINT for a compatible endpoint",
        ));
    }
    let local = matches!(provider, "ollama" | "lmstudio");
    // Environment credentials are accepted only in explicit CI mode; desktop storage remains keychain-only.
    let secrets: Arc<dyn SecretStore> =
        if std::env::var("CI").is_ok_and(|v| v == "true" || v == "1") {
            Arc::new(vellum_secrets_keyring::EnvSecrets::with_lookup(|name| {
                std::env::var(name).ok().or_else(|| {
                    name.strip_prefix("VELLUM_")
                        .and_then(|short| std::env::var(short).ok())
                })
            }))
        } else {
            Arc::new(vellum_secrets_keyring::Keychain::new(
                vellum_types::brand::KEYCHAIN_SERVICE,
            ))
        };
    if !local && secrets.get(provider).map_err(io::Error::other)?.is_none() {
        return Err(io::Error::other(format!(
            "No key for {provider}; configure the OS keychain, or inject VELLUM_{}_API_KEY in CI",
            provider.replace('-', "_").to_ascii_uppercase()
        )));
    }
    let config = ModelConfig {
        id: provider.into(),
        endpoint,
        secret_key: (!local).then(|| provider.into()),
        caps: ModelCaps {
            tools: true,
            vision: args.vision,
            reasoning: args.effort.is_some(),
            context_tokens: args.context_window,
        },
    };
    let transport = Arc::new(vellum_model_http::HttpTransport::default());
    let provider: Arc<dyn ModelProvider> = match provider {
        "openai" => Arc::new(
            vellum_llm_openai_responses::Provider::new(config, secrets, transport)
                .map_err(io::Error::other)?,
        ),
        "anthropic" => Arc::new(
            vellum_llm_anthropic::Provider::new(
                config,
                vellum_llm_anthropic::Deployment::Api,
                secrets,
                transport,
            )
            .map_err(io::Error::other)?,
        ),
        "google" => Arc::new(
            vellum_llm_google::Provider::new(config, secrets, transport)
                .map_err(io::Error::other)?,
        ),
        _ => Arc::new(
            vellum_llm_openai_compat::Provider::new(config, secrets, transport)
                .map_err(io::Error::other)?,
        ),
    };
    Ok((
        Box::new(vellum_engine_builtin::BuiltinEngine::new(
            provider,
            vellum_engine_builtin::PromptPack::builtin(),
        )),
        model.into(),
    ))
}
fn find_program(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|dir| {
            dir.join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.into()
            })
        })
        .find(|p| p.is_file())
}
pub fn engine_status() -> Value {
    json!({"builtin":"requires configured provider","claude-code":if find_program("claude").is_some(){"installed; authentication unchecked"}else{"missing"},"codex":if find_program("codex").is_some(){"installed; authentication unchecked"}else{"missing"}})
}

fn step_line(name: &str, failed: bool, step: Option<&Value>) -> String {
    let symbol = if failed { "✗" } else { "✓" };
    let Some(step) = step else {
        return format!("{symbol} {name}");
    };
    let verb = match step["verb"].as_str().unwrap_or("") {
        "designed" => "Designed",
        "updated" => "Updated",
        "deleted" => "Removed",
        "copied" => "Copied",
        "moved" => "Moved",
        "opened" => "Opened",
        "ran_script" => "Ran script",
        "checked_layout" => "Checked layout",
        "checked_screenshot" => "Checked screenshot",
        "updated_variables" => "Updated variables",
        "added_asset" => "Added asset",
        "replaced" => "Replaced",
        "made_component" => "Made component",
        "made_slot" => "Made slot",
        "claimed" => "Claimed",
        "released" => "Released",
        "progress" => "Progress",
        _ => "Read",
    };
    let target = step["target"]["name"].as_str().unwrap_or(name);
    let layers = ["created", "changed", "removed"]
        .iter()
        .filter_map(|key| step["counts"][key].as_u64())
        .sum::<u64>();
    let ms = step["durationMs"].as_i64().unwrap_or(0).max(0);
    let seconds = if ms % 1000 == 0 {
        (ms / 1000).to_string()
    } else {
        format!("{:.1}", ms as f64 / 1000.0)
    };
    format!("{symbol} {verb} {target} · {layers} layers · {seconds} s")
}

#[cfg(test)]
mod tests {
    //! TEST DATA. Stable terminal step output, independent of native font rendering or a live clock.
    use super::*;
    #[test]
    fn terminal_step_golden() {
        let step = json!({"verb":"designed","target":{"name":"Step 2"},"counts":{"created":31},"durationMs":18000});
        assert_eq!(
            step_line("insert_nodes", false, Some(&step)),
            "✓ Designed Step 2 · 31 layers · 18 s"
        );
        assert_eq!(step_line("execute", true, None), "✗ execute");
    }
}
