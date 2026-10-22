//! The `vellum` binary: daemon, CLI and MCP bridge in one.
use clap::{Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf, process::ExitCode};
use vellum_ipc_local::{DaemonConfig, RunDir};
use vellum_types::brand;

#[derive(Parser)]
#[command(name = "vellum", version, about = brand::TAGLINE, arg_required_else_help = true)]
struct Cli {
    /// Private runtime directory (default: ~/.vellum/run).
    #[arg(long, global = true)]
    run_dir: Option<PathBuf>,
    /// Emit machine-readable results and errors.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Anonymous telemetry: off by default. Shows the exact payload fields. No account required.
    Telemetry {
        #[arg(value_parser = ["on","off","status"], default_value="status")]
        action: String,
        #[arg(long)]
        origin: Option<String>,
    },
    /// Configure non-secret image settings; restart the daemon after changing them.
    AssetSettings {
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        segmentation_model: Option<std::path::PathBuf>,
    },
    /// Read a Figma or image-provider key from stdin and save it only in the OS keychain.
    AssetKey {
        #[arg()]
        kind: String,
    },
    /// Native editor sync bridge. --key names an OS-keychain entry, never a credential value.
    Sync {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        room: String,
        #[arg(long)]
        key: String,
    },
    #[command(flatten)]
    User(vellum::cli::commands::UserCommand),
    /// Serve MCP over stdio, bridged to the local daemon socket.
    Mcp,
    /// Run the daemon in the foreground; hosts and MCP start it automatically.
    Daemon {
        #[command(subcommand)]
        action: Option<DaemonAction>,
        /// Opt in to HTTP POST /rpc on this IP address and port.
        #[arg(long)]
        listen: Option<SocketAddr>,
        /// Exact browser Origin allowed on the optional HTTP listener (repeatable).
        #[arg(long, requires = "listen")]
        allow_origin: Vec<String>,
    },
    /// Show authenticated daemon status without starting it.
    Status,
    /// Run one of the tools an MCP client sees: `vellum tool get_nodes --ids frame1`. `vellum tool --help`
    /// lists them.
    #[command(disable_help_flag = true)]
    Tool {
        /// The tool, then its flags.
        #[arg(num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
#[derive(Subcommand)]
enum DaemonAction {
    Stop,
}
fn execute(cli: Cli) -> std::io::Result<ExitCode> {
    if let Command::Sync {
        endpoint,
        room,
        key,
    } = &cli.command
    {
        use vellum_core::ports::SecretStore;
        let room = vellum_types::sync::RoomId::new(room.clone())
            .map_err(|_| std::io::Error::other("Invalid sync room"))?;
        let credential = vellum_secrets_keyring::Keychain::new(brand::KEYCHAIN_SERVICE)
            .get(key)
            .map_err(|_| std::io::Error::other("Sync key unavailable"))?
            .ok_or_else(|| std::io::Error::other("Sync key unavailable"))?;
        vellum_sync_ws::bridge_stdio(endpoint, &room, &credential)
            .map_err(|_| std::io::Error::other("Sync unavailable"))?;
        return Ok(ExitCode::SUCCESS);
    }
    let dir = match cli.run_dir {
        Some(path) => RunDir::new(path)?,
        None => RunDir::for_user()?,
    };
    // The daemon's private folder holds the run directory; recovery copies sit beside it.
    let state = dir
        .path()
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| dir.path().to_path_buf());
    let out = vellum::cli::commands::Output { json: cli.json };
    if !matches!(
        &cli.command,
        Command::Telemetry { .. } | Command::Daemon { .. }
    ) {
        vellum::telemetry::emit(&state, vellum_telemetry_http::Event::CliStarted);
    }
    match cli.command {
        Command::Telemetry { action, origin } => {
            let mut settings = vellum::telemetry::load(&state);
            if action != "status" {
                settings.enabled = action == "on";
                if origin.is_some() {
                    settings.origin = origin;
                }
                vellum::telemetry::save(&state, &settings)?;
            }
            let disclosure = format!(
                "Telemetry {}. If on: version, OS and anonymous CLI/daemon start counts. Never files, prompts, keys or repo paths.",
                if settings.enabled { "on" } else { "off" }
            );
            out.result(
                "telemetry",
                serde_json::json!({
                    "enabled":settings.enabled,"origin":settings.origin,
                    "fields":["event","version","os","count"],
                    "events":["cli_started","daemon_started"],"count":1,
                    "excluded":["files","prompts","keys","repo paths"]
                }),
                &disclosure,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::AssetSettings {
            provider,
            endpoint,
            model,
            segmentation_model,
        } => {
            let mut settings = vellum::asset_settings::AssetSettings::load(&state)?;
            if provider.is_some() || endpoint.is_some() || model.is_some() {
                settings.provider = provider;
                settings.endpoint = endpoint;
                settings.model = model;
            }
            if segmentation_model.is_some() {
                settings.segmentation_model = segmentation_model;
            }
            settings.save(&state)?;
            out.result(
                "asset settings",
                serde_json::to_value(settings)?,
                "Image settings saved. Restart the daemon to apply them.",
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::AssetKey { kind } => {
            use std::io::{IsTerminal, Read};
            if std::io::stdin().is_terminal() {
                return Err(std::io::Error::other(
                    "Pipe the credential from a secret manager. Terminal input is disabled so the key is not echoed.",
                ));
            }
            let mut input = String::new();
            std::io::stdin().take(4098).read_to_string(&mut input)?;
            vellum::asset_settings::save_key(
                &vellum_secrets_keyring::Keychain::new(brand::KEYCHAIN_SERVICE),
                &kind,
                &input,
            )?;
            out.result(
                "asset key",
                serde_json::json!({"stored":true}),
                "Key stored in the OS keychain.",
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Sync { .. } => unreachable!("sync returns before daemon setup"),
        Command::User(command) => vellum::cli::commands::run(command, &dir, &out).map(|ok| {
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }),
        Command::Mcp => {
            vellum_ipc_local::bridge(&dir, &std::env::current_exe()?).map(|()| ExitCode::SUCCESS)
        }
        Command::Status => {
            let mut value = vellum_ipc_local::status(&dir)?;
            value["engines"] = vellum::cli::prompt::engine_status();
            out.result("status", value, "Daemon connected")?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Tool { mut args } => {
            if cli.json && !args.iter().any(|a| a == "--json") {
                args.insert(0, "--json".into());
            }
            vellum::cli::run(&dir, &std::env::current_exe()?, args)
        }
        Command::Daemon {
            action: Some(DaemonAction::Stop),
            ..
        } => {
            vellum_ipc_local::stop(&dir)?;
            out.result(
                "daemon stop",
                serde_json::json!({"stopped":true}),
                "Daemon stopped",
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Daemon {
            listen,
            allow_origin,
            ..
        } => {
            vellum::telemetry::emit(&state, vellum_telemetry_http::Event::DaemonStarted);
            vellum_ipc_local::run(DaemonConfig {
                dir,
                listen,
                allowed_origins: allow_origin,
                handler: vellum::daemon::production::handler(state),
            })
            .map(|()| ExitCode::SUCCESS)
        }
    }
}
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().collect();
    let json = args.iter().any(|a| a == "--json");
    let out = vellum::cli::commands::Output { json };
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            let code = u8::try_from(error.exit_code()).unwrap_or(2);
            if json {
                if code == 0 {
                    println!(
                        "{}",
                        serde_json::json!({"ok":true,"help":error.to_string()})
                    );
                } else {
                    out.error("arguments", &error.to_string());
                }
            } else {
                let _ = error.print();
            }
            return ExitCode::from(code);
        }
    };
    match execute(cli) {
        Ok(code) => code,
        Err(error) => {
            out.error("vellum", &error.to_string());
            ExitCode::FAILURE
        }
    }
}
