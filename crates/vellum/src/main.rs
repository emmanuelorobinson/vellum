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
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
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
}
#[derive(Subcommand)]
enum DaemonAction {
    Stop,
}
fn execute(cli: Cli) -> std::io::Result<()> {
    let dir = match cli.run_dir {
        Some(path) => RunDir::new(path)?,
        None => RunDir::for_user()?,
    };
    match cli.command {
        Command::Mcp => vellum_ipc_local::bridge(&dir, &std::env::current_exe()?),
        Command::Status => {
            println!("{}", vellum_ipc_local::status(&dir)?);
            Ok(())
        }
        Command::Daemon {
            action: Some(DaemonAction::Stop),
            ..
        } => vellum_ipc_local::stop(&dir),
        Command::Daemon {
            listen,
            allow_origin,
            ..
        } => vellum_ipc_local::run(DaemonConfig {
            dir,
            listen,
            allowed_origins: allow_origin,
        }),
    }
}
fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vellum: {error}");
            ExitCode::FAILURE
        }
    }
}
