//! The `vellum` binary: daemon, CLI and MCP bridge in one.
//!
//! Placeholder: argument parsing only. Nothing here listens on a socket or the network.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use vellum_types::brand;

#[derive(Parser)]
#[command(name = "vellum", version, about = brand::TAGLINE, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve MCP over stdio, bridged to the local daemon socket.
    Mcp,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Mcp => {
            eprintln!("vellum mcp: not implemented yet (the stdio bridge arrives with the daemon)");
            ExitCode::FAILURE
        }
    }
}
