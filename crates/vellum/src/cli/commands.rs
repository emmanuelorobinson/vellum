//! User-facing commands. All canvas calls use one authenticated daemon connection.
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, IsTerminal, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};
use vellum_ipc_local::RunDir;

#[derive(Subcommand)]
pub enum UserCommand {
    /// Connect a cloud account using a browser device code. Local features do not need this.
    Login(super::account::LoginArgs),
    /// Delete this computer's cloud session and request remote revocation.
    Logout,
    /// Show the current cloud account without exposing its session token.
    Whoami,
    /// Run an agent against a document. No account is needed.
    Prompt(Box<super::prompt::PromptArgs>),
    /// List the bundled model hints; installed engines choose their own models.
    Models {
        #[arg(long, default_value="builtin", value_parser=["builtin","claude-code","codex"])]
        engine: String,
    },
    /// Show the CLI version.
    Version,
    /// Create a blank design without replacing an existing file.
    New {
        file: String,
        #[arg(long)]
        system: Option<String>,
    },
    /// Export layers or pages through the shared renderer.
    Export(ExportArgs),
    /// Import or export deterministic design tokens.
    Tokens {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Render changed frames from two committed revisions without changing the checkout.
    Diff(super::git::DiffArgs),
    /// Configure Git integration for the current repo.
    Git {
        #[command(subcommand)]
        action: super::git::GitAction,
    },
    /// Git low-level merge driver: ancestor, current (overwritten), other.
    MergeDriver(super::git::MergeArgs),
    /// Call tools interactively, attached to the editor when available.
    Shell(ShellArgs),
}
#[derive(Args, Clone)]
pub struct ExportArgs {
    pub file: String,
    /// Exact layer/page name or ID (repeatable).
    #[arg(long)]
    pub frame: Vec<String>,
    #[arg(long, default_value="png", value_parser=["png","jpeg","webp","svg","pdf","html-css","html-tailwind"])]
    pub format: String,
    #[arg(long, default_value_t=1, value_parser=clap::value_parser!(u8).range(1..=3))]
    pub scale: u8,
    #[arg(long)]
    pub scaffold: bool,
    #[arg(short, long)]
    pub output: PathBuf,
}
#[derive(Subcommand)]
pub enum TokenAction {
    Export {
        file: String,
        #[arg(long, default_value="tailwind-v4", value_parser=["css-vars","tailwind-v4","dtcg-json"])]
        format: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Import {
        file: String,
        source: PathBuf,
        #[arg(long, default_value="css-vars", value_parser=["css-vars","tailwind-v4","dtcg-json"])]
        format: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        remove_missing: bool,
    },
}
#[derive(Args)]
pub struct ShellArgs {
    pub file: Option<String>,
    #[arg(long)]
    pub headless: bool,
    #[arg(short = 'i', long)]
    pub input: Option<String>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// No ANSI escapes are emitted, including when NO_COLOR is set. Symbols carry the status.
pub struct Output {
    pub json: bool,
}
impl Output {
    pub fn result(&self, command: &str, mut value: Value, text: &str) -> io::Result<()> {
        value["ok"] = json!(true);
        value["command"] = json!(command);
        if self.json {
            println!("{value}");
        } else {
            println!("✓ {text}");
        }
        io::stdout().flush()
    }
    pub fn error(&self, command: &str, message: &str) {
        if self.json {
            println!(
                "{}",
                json!({"ok":false,"command":command,"error":{"message":message}})
            );
        } else {
            eprintln!("✗ {message}");
        }
    }
}

/// A child runs the existing stdio bridge, preserving the daemon's session, default document and attribution.
/// The child has no shell and never receives model credentials in its argv.
pub struct Rpc {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    next: u64,
    pub step: Option<Value>,
    preview_root: PathBuf,
    pub(crate) run_dir: RunDir,
    pub(crate) lane: Value,
}
impl Rpc {
    pub fn connect(dir: &RunDir) -> io::Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .args(["--run-dir"])
            .arg(dir.path())
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000200);
        }
        let mut child = command.spawn()?;
        let input = child.stdin.take();
        let output = BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("Missing bridge output"))?,
        );
        let mut rpc = Self {
            child,
            input,
            output,
            next: 0,
            step: None,
            preview_root: dir.path().join("previews"),
            run_dir: dir.clone(),
            lane: Value::Null,
        };
        rpc.request("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"vellum-cli","version":env!("CARGO_PKG_VERSION")}}))?;
        Ok(rpc)
    }
    pub fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.next += 1;
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| io::Error::other("Bridge is closed"))?;
        let bytes = serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params}),
        )
        .map_err(io::Error::other)?;
        if bytes.len() + 1 > 1024 * 1024 {
            return Err(io::Error::other(
                "Request exceeds the daemon's 1 MiB message budget; reduce attachments or tool output",
            ));
        }
        input.write_all(&bytes)?;
        input.write_all(b"\n")?;
        input.flush()?;
        let mut line = String::new();
        if read_line(&mut self.output, &mut line, 16 * 1024 * 1024)? == 0 {
            return Err(io::Error::other(
                "Daemon disconnected; run vellum status and retry",
            ));
        }
        let reply: Value = serde_json::from_str(&line).map_err(io::Error::other)?;
        if reply["id"] != self.next {
            return Err(io::Error::other("Unexpected daemon reply"));
        }
        if let Some(error) = reply.get("error") {
            return Err(io::Error::other(
                error["message"]
                    .as_str()
                    .unwrap_or("Daemon refused request"),
            ));
        }
        let result = reply
            .get("result")
            .cloned()
            .ok_or_else(|| io::Error::other("Missing daemon result"))?;
        if method == "vellum/cli" && result.get("agent").is_some() {
            self.lane = result.clone();
        }
        Ok(result)
    }
    pub fn tool(&mut self, name: &str, arguments: Value) -> io::Result<Value> {
        let result = self.request("tools/call", json!({"name":name,"arguments":arguments}))?;
        self.step = result.pointer("/_meta/vellum~1step").cloned();
        if result["isError"] == true {
            return Err(io::Error::other(
                result["content"][0]["text"]
                    .as_str()
                    .unwrap_or("Tool failed"),
            ));
        }
        let mut value = result["structuredContent"].clone();
        if let Some(image) = result["content"]
            .as_array()
            .and_then(|rows| rows.iter().find(|c| c["type"] == "image"))
        {
            value["image"]["base64"] = image["data"].clone();
        }
        Ok(value)
    }
    pub fn open(&mut self, file: &str, create: bool) -> io::Result<Value> {
        self.tool("open_document", json!({"path":file,"create":create}))
    }
    pub fn preview_file(&self, bytes: &[u8]) -> io::Result<PathBuf> {
        let folder = RunDir::new(self.preview_root.clone())?;
        let path = folder
            .path()
            .join(format!("{}-{}.png", self.child.id(), self.next));
        std::fs::write(&path, bytes)?;
        Ok(path)
    }
    pub fn save(&mut self, file: &str) -> io::Result<Value> {
        self.request("vellum/cli", json!({"action":"save","document":file}))
    }
}
impl Drop for Rpc {
    fn drop(&mut self) {
        self.input.take();
        // A normal EOF drains the existing bridge. A failed bridge is killed and always reaped.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while self.child.try_wait().ok().flatten().is_none() && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn run(command: UserCommand, dir: &RunDir, out: &Output) -> io::Result<bool> {
    match command {
        UserCommand::Version => out.result(
            "version",
            json!({"version":env!("CARGO_PKG_VERSION")}),
            &format!("vellum {}", env!("CARGO_PKG_VERSION")),
        )?,
        UserCommand::Models { engine } => {
            let models = if engine == "builtin" {
                vec!["openrouter/deepseek/deepseek-v4.1-flash"]
            } else {
                vec![]
            };
            out.result("models",json!({"engine":engine,"models":models,"complete":false,"source":"bundled","note":"Supply --model for the configured provider or installed engine; this is not a live catalog."}), &format!("{engine}: {} · bundled hints; supply --model for other models",models.join(", ")))?;
        }
        UserCommand::New { file, system } => {
            if system.is_some() {
                return Err(io::Error::other(
                    "Design-system catalog is unavailable; omit --system to create a blank design",
                ));
            }
            let mut rpc = Rpc::connect(dir)?;
            let created = rpc.request("vellum/cli", json!({"action":"new","document":file}))?;
            out.result("new", created, &format!("Created {file}"))?;
        }
        UserCommand::Export(args) => {
            let mut rpc = Rpc::connect(dir)?;
            let result = export(&mut rpc, &args)?;
            out.result(
                "export",
                result,
                &format!("Exported {}", args.output.display()),
            )?;
        }
        UserCommand::Tokens { action } => {
            let mut rpc = Rpc::connect(dir)?;
            match action {
                TokenAction::Export {
                    file,
                    format,
                    output,
                } => {
                    rpc.open(&file, false)?;
                    let value =
                        rpc.tool("export_tokens", json!({"document":file,"format":format}))?;
                    let text = value["text"]
                        .as_str()
                        .ok_or_else(|| io::Error::other("No token output"))?;
                    if let Some(path) = output {
                        std::fs::write(&path, text)?;
                        out.result("tokens export",json!({"output":path,"tokens":value["tokens"],"warnings":value["warnings"]}),&format!("Exported {}",path.display()))?;
                    } else if out.json {
                        out.result("tokens export", value, "")?;
                    } else {
                        print!("{text}");
                        io::stdout().flush()?;
                    }
                }
                TokenAction::Import {
                    file,
                    source,
                    format,
                    dry_run,
                    remove_missing,
                } => {
                    let text = std::fs::read_to_string(&source)?;
                    rpc.open(&file, false)?;
                    let value=rpc.tool("import_tokens",json!({"document":file,"format":format,"text":text,"dry_run":dry_run,"remove_missing":remove_missing}))?;
                    if !dry_run {
                        rpc.save(&file)?;
                    }
                    out.result(
                        "tokens import",
                        value,
                        if dry_run {
                            "Checked token import"
                        } else {
                            "Imported tokens"
                        },
                    )?;
                }
            }
        }
        UserCommand::Login(args) => return super::account::login(args, out),
        UserCommand::Logout => return super::account::logout(out),
        UserCommand::Whoami => return super::account::whoami(out),
        UserCommand::Diff(args) => return super::git::diff(args, out),
        UserCommand::Git {
            action: super::git::GitAction::Setup,
        } => return super::git::setup(out),
        UserCommand::MergeDriver(args) => return super::git::merge_driver(args, out),
        UserCommand::Shell(args) => return shell(dir, out, args),
        UserCommand::Prompt(args) => return super::prompt::run(dir, out, *args),
    }
    Ok(true)
}

pub fn export(rpc: &mut Rpc, args: &ExportArgs) -> io::Result<Value> {
    let opened = rpc.open(&args.file, false)?;
    let mut nodes = vec![];
    if args.frame.is_empty() {
        nodes = opened["pages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p["id"].as_str().map(str::to_owned))
            .collect();
    } else {
        for wanted in &args.frame {
            // Use an exact ID first, then require exactly one exact name (the query API itself uses substrings).
            if rpc
                .tool(
                    "get_nodes",
                    json!({"document":args.file,"ids":[wanted],"depth":0}),
                )
                .is_ok_and(|v| {
                    v["nodes"]
                        .as_array()
                        .is_some_and(|nodes| nodes.iter().any(|n| n["id"] == *wanted))
                })
            {
                nodes.push(wanted.clone());
                continue;
            }
            let queried = rpc.tool(
                "query_nodes",
                json!({"document":args.file,"name":wanted,"limit":500}),
            )?;
            if queried["truncated"] == true || queried["total"].as_u64().is_some_and(|n| n > 500) {
                return Err(io::Error::other(
                    "Layer query is truncated; use an exact ID",
                ));
            }
            let mut found: Vec<String> = queried["nodes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|n| n["name"] == *wanted)
                .filter_map(|n| n["id"].as_str().map(str::to_owned))
                .collect();
            found.extend(
                opened["pages"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["name"] == *wanted)
                    .filter_map(|p| p["id"].as_str().map(str::to_owned)),
            );
            if found.len() != 1 {
                return Err(io::Error::other(format!(
                    "Expected one layer/page named {wanted:?}; found {}. Use an exact ID",
                    found.len()
                )));
            }
            nodes.push(found.remove(0));
        }
    }
    let result=rpc.tool("export",json!({"document":args.file,"nodes":nodes,"format":args.format,"scale":args.scale,"scaffold":args.scaffold}))?;
    let files = result["files"]
        .as_array()
        .ok_or_else(|| io::Error::other("Export returned no files"))?;
    if files.len() != 1 {
        return Err(io::Error::other(
            "Export returned several files; choose a single frame or an inline format",
        ));
    }
    let bytes = files[0]["base64"]
        .as_str()
        .and_then(vellum_core::base64::decode)
        .ok_or_else(|| io::Error::other("Invalid export bytes"))?;
    std::fs::write(&args.output, &bytes)?;
    Ok(
        json!({"output":args.output,"format":args.format,"bytes":bytes.len(),"mime":files[0]["mime"],"warnings":result["warnings"]}),
    )
}

fn shell(dir: &RunDir, out: &Output, args: ShellArgs) -> io::Result<bool> {
    if args.file.is_some() && args.input.is_some() {
        return Err(io::Error::other("Choose FILE or --input, not both"));
    }
    if (args.input.is_some() || args.output.is_some()) && !args.headless {
        return Err(io::Error::other("--input/--output require --headless"));
    }
    let mut file = args
        .file
        .or(args.input)
        .ok_or_else(|| io::Error::other("Name a document or use --headless -i INPUT -o OUTPUT"))?;
    let mut rpc = Rpc::connect(dir)?;
    let opened = rpc.open(&file, false)?;
    if args.headless && opened["mode"] != "headless" {
        return Err(io::Error::other(
            "The file has a live writer; close it in the editor before --headless",
        ));
    }
    if let Some(output) = args.output {
        if output.extension().is_none_or(|e| e != "vellum") {
            return Err(io::Error::other("Shell output must end in .vellum"));
        }
        let cwd = std::env::current_dir()?.canonicalize()?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !parent.canonicalize()?.starts_with(&cwd) {
            return Err(io::Error::other(
                "Shell output must be inside the working directory",
            ));
        }
        if Path::new(&file) == output {
            return Err(io::Error::other(
                "Output must differ from input; omit -o to edit in place",
            ));
        }
        // Canonical bytes are copied only for an explicit Save As, before the output is opened as a target.
        let mut source = std::fs::File::open(&file)?;
        let mut destination = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        io::copy(&mut source, &mut destination)?;
        destination.sync_all()?;
        file = output.to_string_lossy().into_owned();
        rpc.open(&file, false)?;
    }
    let interactive = io::stdin().is_terminal() && !out.json;
    if interactive {
        println!(
            "✓ {} · {} · call tools; save(); exit()",
            file,
            opened["mode"].as_str().unwrap_or("unavailable")
        );
    }
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut success = true;
    loop {
        if interactive {
            print!("vellum> ");
            io::stdout().flush()?;
        }
        let mut line = String::new();
        if read_line(&mut input, &mut line, 100_000)? == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if matches!(line, "exit()" | "quit()" | "exit" | "quit") {
            break;
        }
        let name = line.split('(').next().unwrap_or("shell").trim();
        let result = (|| {
            let (name, arguments) = parse_call(line)?;
            if name == "save" {
                if arguments != json!({}) {
                    return Err(io::Error::other("save() takes no arguments"));
                }
                rpc.save(&file)
            } else {
                rpc.tool(&name, arguments)
            }
        })();
        match result {
            Ok(value) => {
                if name == "open_document"
                    && let Some(path) = value["path"].as_str()
                {
                    file = path.to_owned();
                }
                out.result(name, value.clone(), &format!("{name} complete"))?;
                if !out.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value).map_err(io::Error::other)?
                    );
                }
            }
            Err(error) => {
                success = false;
                out.error(name, &error.to_string());
            }
        }
    }
    rpc.save(&file)?;
    Ok(success)
}

/// JavaScript object literals are decoded inside the existing bounded sandbox, with no document APIs.
pub fn parse_call(line: &str) -> io::Result<(String, Value)> {
    use vellum_core::ports::*;
    struct NoApi;
    impl ScriptApi for NoApi {
        fn node_cost(&self, _: &str, _: &[ScriptValue]) -> Result<u32, ScriptError> {
            Err(ScriptError::OpLimit)
        }
        fn call(&self, _: &str, _: &[ScriptValue]) -> Result<ScriptValue, ScriptError> {
            Err(ScriptError::OpLimit)
        }
    }
    if line.len() > 100_000 {
        return Err(io::Error::other("Shell call exceeds 100,000 bytes"));
    }
    let (name, rest) = line
        .trim_end_matches(';')
        .split_once('(')
        .ok_or_else(|| io::Error::other("Use tool_name({arguments}) or save()"))?;
    let name = name.trim();
    if name.is_empty() || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
        return Err(io::Error::other("Invalid tool name"));
    }
    let args = rest
        .strip_suffix(')')
        .ok_or_else(|| io::Error::other("Close the call with )"))?
        .trim();
    if args.is_empty() {
        return Ok((name.into(), json!({})));
    }
    let runtime = crate::daemon::production::script_runtime()
        .ok_or_else(|| io::Error::other("Shell object literals require the scripts feature"))?;
    let result = runtime
        .run(
            &format!("return ({args});"),
            std::rc::Rc::new(NoApi),
            ScriptLimits {
                time_ms: 100,
                max_ops: 0,
                max_nodes: 0,
                memory_mb: 8,
            },
        )
        .map_err(io::Error::other)?;
    let value = vellum_core::doc_api::to_json(&result.value).map_err(io::Error::other)?;
    if !value.is_object() {
        return Err(io::Error::other("Tool arguments must be an object"));
    }
    Ok((name.into(), value))
}

fn read_line(reader: &mut impl BufRead, line: &mut String, limit: usize) -> io::Result<usize> {
    let mut bytes = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let n = buffer
            .iter()
            .position(|b| *b == b'\n')
            .map_or(buffer.len(), |n| n + 1);
        if bytes.len() + n > limit {
            return Err(io::Error::other("Input exceeds the line budget"));
        }
        bytes.extend_from_slice(&buffer[..n]);
        reader.consume(n);
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    let length = bytes.len();
    *line = String::from_utf8(bytes).map_err(io::Error::other)?;
    Ok(length)
}
