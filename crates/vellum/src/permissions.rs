//! Doing things outside the canvas (ARCHITECTURE §7.7, guide Invariant 11), on this computer.
//!
//! The decision is `vellum_core::permissions::Broker`'s. This module is the other half: the only code in the
//! binary that reads or writes a file, or runs a command, for an agent. Each function takes the
//! [`Permit`] the broker gave for exactly that action, so an action without an `Allowed` record cannot be written.
//! Paths are resolved before asking ([`read_action`], [`write_action`]): symbolic links are followed, so a link in
//! the repository pointing outside it is asked about as the outside file it is.
//!
//! Also here: the terminal prompt the CLI asks with, and the `--allow` / `--deny` / `--skip-permissions` flags.
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use vellum_core::permissions::{Effect, PermissionSettings, Permit, Rule, RuleError};
use vellum_core::ports::{
    Action, Answer, EngineFuture, PermissionBroker, PermissionKind, PermissionPrompt,
    PermissionRequest, Question,
};
use vellum_types::Origin;

use crate::tools::{ErrorKind, ToolError};

/// Why an outside action did not happen.
#[derive(Debug)]
pub enum OutsideError {
    /// The permit is for something else.
    NotPermitted,
    Io(io::Error),
}

impl std::fmt::Display for OutsideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPermitted => f.write_str("the permission given is for another action"),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OutsideError {}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Where `path` really is: the file itself resolved, or for a file that does not exist yet, its folder resolved.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(real) => Ok(real),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
            let folder = path.parent().filter(|p| !p.as_os_str().is_empty());
            Ok(folder.unwrap_or(Path::new(".")).canonicalize()?.join(name))
        }
        Err(error) => Err(error),
    }
}

/// The action reading `path` is, to ask the broker about.
pub fn read_action(path: &Path) -> io::Result<Action> {
    Ok(Action::ReadFile {
        path: text(&resolve(path)?),
    })
}

/// The action writing `path` is, to ask the broker about.
pub fn write_action(path: &Path) -> io::Result<Action> {
    Ok(Action::WriteFile {
        path: text(&resolve(path)?),
    })
}

/// The action running `program` with `args` in `cwd` is. Nothing goes through a shell.
pub fn run_action(program: &str, args: &[String], cwd: &Path) -> io::Result<Action> {
    Ok(Action::RunCommand {
        program: program.to_owned(),
        args: args.to_vec(),
        cwd: text(&cwd.canonicalize()?),
    })
}

/// Checks the permit against what is about to happen, resolved again now.
fn check(permit: &Permit, now: io::Result<Action>) -> Result<(), OutsideError> {
    let now = now.map_err(OutsideError::Io)?;
    if permit.covers(&now) {
        Ok(())
    } else {
        Err(OutsideError::NotPermitted)
    }
}

/// Reads `path` with the permit for reading it.
pub fn read_file(permit: Permit, path: &Path) -> Result<Vec<u8>, OutsideError> {
    check(&permit, read_action(path))?;
    std::fs::read(path).map_err(OutsideError::Io)
}

/// Writes `bytes` to `path` with the permit for writing it: a temporary file beside it, then a rename.
pub fn write_file(permit: Permit, path: &Path, bytes: &[u8]) -> Result<(), OutsideError> {
    check(&permit, write_action(path))?;
    let name = path
        .file_name()
        .ok_or_else(|| OutsideError::Io(io::Error::from(io::ErrorKind::InvalidInput)))?;
    let temp = path.with_file_name(format!(".{}.vellum-write", name.to_string_lossy()));
    std::fs::write(&temp, bytes)
        .and_then(|()| std::fs::rename(&temp, path))
        .map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            OutsideError::Io(error)
        })
}

/// Runs the command with the permit for running it, and waits for it.
pub fn run_command(
    permit: Permit,
    program: &str,
    args: &[String],
    cwd: &Path,
) -> Result<std::process::Output, OutsideError> {
    check(&permit, run_action(program, args, cwd))?;
    Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(OutsideError::Io)
}

/// What a tool call may ask of the world for the agent that made it. Each method asks the permission broker
/// first and acts only on a recorded allow; a refusal comes back as a [`ToolError`] of kind `Refused` that says
/// what was not allowed and why.
pub trait Outside {
    /// The bytes of the file at `path`, at most `max_bytes` of them.
    fn read_file(&self, origin: &Origin, path: &Path, max_bytes: u64)
    -> Result<Vec<u8>, ToolError>;
    /// Whether the agent may reach `url` (an origin such as `https://pixabay.com`). `Ok` means allowed and
    /// recorded; whoever then makes the request is the caller.
    fn network(&self, origin: &Origin, url: &str) -> Result<(), ToolError>;
}

/// [`Outside`] decided by a [`PermissionBroker`]. `engine` is the installed engine the turn runs on, or `None`
/// for an external client that called the tool itself.
pub struct Brokered {
    broker: std::rc::Rc<dyn PermissionBroker>,
    engine: Option<String>,
}

impl Brokered {
    pub fn new(broker: std::rc::Rc<dyn PermissionBroker>, engine: Option<String>) -> Self {
        Self { broker, engine }
    }

    fn permit(&self, origin: &Origin, action: Action) -> Result<Permit, ToolError> {
        let request = PermissionRequest {
            origin: origin.clone(),
            action,
            engine: self.engine.clone(),
            engine_verdict: None,
        };
        block_on(self.broker.request(request))
            .map_err(|refusal| ToolError::new(ErrorKind::Refused, refusal.to_string()))
    }
}

/// Waits for a future that is answered by the person at this computer (a terminal line, a card in the panel).
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

impl Outside for Brokered {
    fn read_file(
        &self,
        origin: &Origin,
        path: &Path,
        max_bytes: u64,
    ) -> Result<Vec<u8>, ToolError> {
        let action = read_action(path).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => {
                ToolError::not_found(format!("There is no file at {}.", path.display()))
            }
            _ => ToolError::new(
                ErrorKind::Document,
                format!("{} cannot be read: {e}.", path.display()),
            ),
        })?;
        // Asked first, so a missing file outside the repository is not told apart from one the person denied.
        let permit = self.permit(origin, action)?;
        let unreadable = |e: io::Error| match e.kind() {
            io::ErrorKind::NotFound => {
                ToolError::not_found(format!("There is no file at {}.", path.display()))
            }
            _ => ToolError::new(
                ErrorKind::Document,
                format!("{} cannot be read: {e}.", path.display()),
            ),
        };
        let size = std::fs::metadata(path)
            .map(|m| m.len())
            .map_err(unreadable)?;
        if size > max_bytes {
            return Err(ToolError::invalid(
                "path",
                format!(
                    "{} is {} KB; at most {} KB can be used.",
                    path.display(),
                    size / 1024,
                    max_bytes / 1024
                ),
            ));
        }
        read_file(permit, path).map_err(|e| match e {
            OutsideError::Io(e) => unreadable(e),
            other => ToolError::new(
                ErrorKind::Document,
                format!("{} cannot be read: {other}.", path.display()),
            ),
        })
    }

    fn network(&self, origin: &Origin, url: &str) -> Result<(), ToolError> {
        self.permit(
            origin,
            Action::Network {
                url: url.to_owned(),
            },
        )
        .map(drop)
    }
}

/// Settings from the command line: `--allow <rule>` and `--deny <rule>` (each `read|write|run <pattern>`), and
/// `--skip-permissions`.
pub fn settings_from_flags(
    allow: &[String],
    deny: &[String],
    skip_all: bool,
) -> Result<PermissionSettings, RuleError> {
    let mut rules = vec![];
    for text in deny {
        rules.push(Rule::parse(Effect::Deny, text)?);
    }
    for text in allow {
        rules.push(Rule::parse(Effect::Allow, text)?);
    }
    Ok(PermissionSettings {
        skip_all,
        rules,
        ..PermissionSettings::default()
    })
}

/// What the terminal prints while "Skip all permission prompts" is on, before every turn.
pub const SKIP_ALL_WARNING: &str =
    "Permission prompts are off: agents can read and write files and run commands without asking.";

/// The sentence a question is asked with, naming the exact path, command or address.
pub fn sentence(question: &Question, agent: &str) -> String {
    match &question.action {
        Action::ReadFile { path } => format!("{agent} wants to read `{path}`"),
        Action::WriteFile { path } => format!("{agent} wants to write `{path}`"),
        Action::RunCommand { program, args, cwd } => format!(
            "{agent} wants to run `{}` in `{cwd}`",
            Action::command_line(program, args)
        ),
        Action::Network { url } => format!("{agent} wants to reach `{url}`"),
    }
}

/// Asks in the terminal: one line per question, answered `o` (Allow once), `s` (Allow for this session, when
/// offered) or `d` (Deny). End of input is a denial; anything else asks again.
pub struct TerminalPrompt<R, W> {
    agent: String,
    io: Mutex<(R, W)>,
}

impl<R: BufRead, W: Write> TerminalPrompt<R, W> {
    /// Asks on `output`, reads answers from `input`; `agent` is how the agent is named ("Teal", "Claude Code").
    pub fn new(agent: impl Into<String>, input: R, output: W) -> Self {
        Self {
            agent: agent.into(),
            io: Mutex::new((input, output)),
        }
    }

    fn ask_now(&self, question: &Question) -> Option<Answer> {
        let mut io = self.io.lock().ok()?;
        let (input, output) = &mut *io;
        let options: Vec<&str> = question
            .offered
            .iter()
            .map(|answer| match answer {
                Answer::AllowOnce => "[o] Allow once",
                Answer::AllowForSession => "[s] Allow for this session",
                Answer::Deny => "[d] Deny",
            })
            .collect();
        loop {
            writeln!(output, "{}", sentence(question, &self.agent)).ok()?;
            if matches!(
                question.kind,
                PermissionKind::ReadOutside | PermissionKind::WriteOutside
            ) {
                writeln!(output, "This is outside the repository.").ok()?;
            }
            write!(output, "{}: ", options.join("  ")).ok()?;
            output.flush().ok()?;
            let mut line = String::new();
            if input.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let answer = match line.trim().to_ascii_lowercase().as_str() {
                "o" | "once" => Some(Answer::AllowOnce),
                "s" | "session" => Some(Answer::AllowForSession),
                "d" | "deny" => Some(Answer::Deny),
                _ => None,
            };
            if let Some(answer) = answer.filter(|a| question.offered.contains(a)) {
                return Some(answer);
            }
        }
    }
}

impl<R: BufRead, W: Write> PermissionPrompt for TerminalPrompt<R, W> {
    fn ask<'a>(&'a self, question: Question) -> EngineFuture<'a, Option<Answer>> {
        let answer = self.ask_now(&question);
        Box::pin(async move { answer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_contracts::suites::model::block_on;

    fn question(action: Action, kind: PermissionKind) -> Question {
        Question {
            agent: None,
            engine: Some("builtin".into()),
            offered: if kind.session_grant_offered() {
                vec![Answer::AllowOnce, Answer::AllowForSession, Answer::Deny]
            } else {
                vec![Answer::AllowOnce, Answer::Deny]
            },
            action,
            kind,
        }
    }

    fn ask(input: &str, q: Question) -> (Option<Answer>, String) {
        let mut out = Vec::new();
        let answer = {
            let prompt = TerminalPrompt::new("Teal", input.as_bytes(), &mut out);
            block_on(prompt.ask(q))
        };
        (answer, String::from_utf8(out).unwrap())
    }

    #[test]
    fn the_terminal_names_the_exact_action_and_reads_the_answer() {
        let read = question(
            Action::ReadFile {
                path: "/TEST DATA/juniper/DESIGN.md".into(),
            },
            PermissionKind::ReadRepo,
        );
        let (answer, out) = ask("s\n", read.clone());
        assert_eq!(answer, Some(Answer::AllowForSession));
        assert!(
            out.starts_with("Teal wants to read `/TEST DATA/juniper/DESIGN.md`\n"),
            "{out}"
        );
        assert!(out.contains("[o] Allow once  [s] Allow for this session  [d] Deny: "));
        assert_eq!(ask("maybe\nd\n", read.clone()).0, Some(Answer::Deny));
        assert_eq!(ask("", read).0, None, "end of input denies");
    }

    #[test]
    fn the_terminal_does_not_take_an_answer_it_did_not_offer() {
        let outside = question(
            Action::ReadFile {
                path: "/etc/hosts".into(),
            },
            PermissionKind::ReadOutside,
        );
        let (answer, out) = ask("s\no\n", outside);
        assert_eq!(answer, Some(Answer::AllowOnce));
        assert!(!out.contains("[s]"));
        assert!(out.contains("This is outside the repository."));
        let run = question(
            Action::RunCommand {
                program: "pnpm".into(),
                args: vec!["build".into()],
                cwd: "/TEST DATA/juniper".into(),
            },
            PermissionKind::RunCommand,
        );
        assert!(
            ask("o\n", run)
                .1
                .starts_with("Teal wants to run `pnpm build` in `/TEST DATA/juniper`")
        );
    }

    #[test]
    fn flags_become_rules_with_deny_first() {
        let settings =
            settings_from_flags(&["read docs/**".into()], &["run rm *".into()], false).unwrap();
        assert_eq!(
            settings
                .rules
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["deny run rm *", "allow read docs/**"]
        );
        assert!(!settings.skip_all);
        assert!(settings_from_flags(&["delete x".into()], &[], false).is_err());
        assert!(
            settings_from_flags(&[], &[], true)
                .unwrap()
                .shows_skip_all_warning()
        );
    }
}
