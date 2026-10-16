//! The pure half of the broker: what kind of action a request is, which rules apply, and what is decided without
//! asking anyone. No store, no clock.
use crate::ports::{
    Action, Answer, DecisionSource, EventBody, PermissionKind, PermissionRecord, SessionEvent,
};
use std::collections::BTreeSet;
use std::fmt;

/// Ask or Allow, as Settings → Permissions offers for repo reads, file writes and commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Setting {
    #[default]
    Ask,
    Allow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Allow,
    Deny,
}

/// What a rule is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTarget {
    Read,
    Write,
    Run,
}

/// A pre-decided answer, from settings or `--allow` / `--deny` on the command line: `read docs/**`,
/// `write tokens/*.css`, `run pnpm build`.
///
/// Read and write patterns match the repo-relative path (`/`-separated) of a file in the repository, or the
/// absolute path of one outside it when the pattern starts with `/` or a drive. Run patterns match the command
/// line (program and arguments, separated by spaces). `**` matches anything; `*` and `?` stop at `/` in paths
/// and match anything in a command line.
///
/// A deny rule always wins. An allow rule covers only repo reads, repo writes and commands: reading or writing
/// outside the repository and network access are asked every time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub effect: Effect,
    pub target: RuleTarget,
    pub pattern: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleError(pub String);
impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for RuleError {}

impl Rule {
    /// Reads `read <pattern>`, `write <pattern>` or `run <pattern>`.
    pub fn parse(effect: Effect, text: &str) -> Result<Self, RuleError> {
        let text = text.trim();
        let (word, pattern) = text.split_once(char::is_whitespace).ok_or_else(|| {
            RuleError(format!(
                "`{text}` is not a rule. Write `read <pattern>`, `write <pattern>` or `run <pattern>`."
            ))
        })?;
        let target = match word {
            "read" => RuleTarget::Read,
            "write" => RuleTarget::Write,
            "run" => RuleTarget::Run,
            other => {
                return Err(RuleError(format!(
                    "`{other}` is not read, write or run in the rule `{text}`."
                )));
            }
        };
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return Err(RuleError(format!("The rule `{text}` has no pattern.")));
        }
        Ok(Self {
            effect,
            target,
            pattern: pattern.to_owned(),
        })
    }

    fn matches(&self, request: &Classified) -> bool {
        match (&self.target, &request.subject) {
            (RuleTarget::Read, Subject::Read(path)) | (RuleTarget::Write, Subject::Write(path)) => {
                let absolute = is_absolute(&self.pattern);
                match (&path.inside, absolute) {
                    (Some(relative), false) => glob(&self.pattern, relative, true),
                    (_, true) => glob(&normalize_slashes(&self.pattern), &path.absolute, true),
                    (None, false) => false,
                }
            }
            (RuleTarget::Run, Subject::Run(line)) => glob(&self.pattern, line, false),
            _ => false,
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let effect = match self.effect {
            Effect::Allow => "allow",
            Effect::Deny => "deny",
        };
        let target = match self.target {
            RuleTarget::Read => "read",
            RuleTarget::Write => "write",
            RuleTarget::Run => "run",
        };
        write!(f, "{effect} {target} {}", self.pattern)
    }
}

/// Settings → Permissions (the `[permissions]` table of the settings file) plus rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionSettings {
    pub read_repo: Setting,
    pub write_files: Setting,
    pub run_commands: Setting,
    /// "Skip all permission prompts". Off by default; while on, every host shows a persistent warning.
    pub skip_all: bool,
    /// Installed engines whose own permission system decides for them (their auto mode, turned on by the person).
    pub engine_auto: BTreeSet<String>,
    /// Checked in order; any matching deny wins over everything else.
    pub rules: Vec<Rule>,
}

impl PermissionSettings {
    /// Whether the persistent "Skip all" warning must be shown.
    pub fn shows_skip_all_warning(&self) -> bool {
        self.skip_all
    }
}

/// A path as the policy sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathInRepo {
    /// Normalised absolute path, `/`-separated.
    pub absolute: String,
    /// The path inside the repository, when it is inside.
    pub inside: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Subject {
    Read(PathInRepo),
    Write(PathInRepo),
    Run(String),
    Network,
}

/// A request's action, sorted into its kind against the session's repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classified {
    pub kind: PermissionKind,
    subject: Subject,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassifyError {
    /// A path (or the repository root) is not absolute, or `..` climbs above its root.
    NotAbsolute(String),
}
impl fmt::Display for ClassifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAbsolute(path) => write!(f, "{path} is not an absolute path"),
        }
    }
}

fn normalize_slashes(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("//?/").map(str::to_owned).unwrap_or(path)
}

fn is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

/// `path` with `/` separators and no `.` or `..` segments, or `None` when it is not absolute or `..` climbs
/// above its root.
pub fn normalize(path: &str) -> Option<String> {
    let path = normalize_slashes(path);
    if !is_absolute(&path) {
        return None;
    }
    let (prefix, rest) = if let Some(rest) = path.strip_prefix("//") {
        // `//server/share/…`: the server and share are the root.
        let mut parts = rest.splitn(3, '/');
        let server = parts.next().filter(|s| !s.is_empty())?;
        let share = parts.next().filter(|s| !s.is_empty())?;
        (
            format!("//{server}/{share}"),
            parts.next().unwrap_or("").to_owned(),
        )
    } else if let Some(rest) = path.strip_prefix('/') {
        (String::new(), rest.to_owned())
    } else {
        (path[..2].to_owned(), path[3..].to_owned())
    };
    let mut parts: Vec<&str> = vec![];
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    Some(format!("{prefix}/{}", parts.join("/")))
}

fn locate(path: &str, root: &str) -> Result<PathInRepo, ClassifyError> {
    let absolute = normalize(path).ok_or_else(|| ClassifyError::NotAbsolute(path.to_owned()))?;
    let root = normalize(root).ok_or_else(|| ClassifyError::NotAbsolute(root.to_owned()))?;
    let base = if root.ends_with('/') {
        root.clone()
    } else {
        format!("{root}/")
    };
    let inside = absolute
        .strip_prefix(&base)
        .filter(|rest| !rest.is_empty())
        .map(str::to_owned);
    Ok(PathInRepo { absolute, inside })
}

/// Sorts `action` into its kind, for a session whose repository is at `repo_root`.
pub fn classify(action: &Action, repo_root: &str) -> Result<Classified, ClassifyError> {
    Ok(match action {
        Action::ReadFile { path } => {
            let path = locate(path, repo_root)?;
            Classified {
                kind: if path.inside.is_some() {
                    PermissionKind::ReadRepo
                } else {
                    PermissionKind::ReadOutside
                },
                subject: Subject::Read(path),
            }
        }
        Action::WriteFile { path } => {
            let path = locate(path, repo_root)?;
            Classified {
                kind: if path.inside.is_some() {
                    PermissionKind::WriteRepo
                } else {
                    PermissionKind::WriteOutside
                },
                subject: Subject::Write(path),
            }
        }
        Action::RunCommand { program, args, cwd } => {
            normalize(cwd).ok_or_else(|| ClassifyError::NotAbsolute(cwd.clone()))?;
            Classified {
                kind: PermissionKind::RunCommand,
                subject: Subject::Run(Action::command_line(program, args)),
            }
        }
        Action::Network { .. } => Classified {
            kind: PermissionKind::Network,
            subject: Subject::Network,
        },
    })
}

/// `text` matches `pattern` (see [`Rule`]). With `paths`, `*` and `?` stop at `/`.
pub fn glob(pattern: &str, text: &str, paths: bool) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    // Iterative with memo: m[i][j] = pattern[i..] matches text[j..].
    let mut memo = vec![vec![None; t.len() + 1]; p.len() + 1];
    fn go(
        p: &[char],
        t: &[char],
        i: usize,
        j: usize,
        paths: bool,
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(known) = memo[i][j] {
            return known;
        }
        let result = if i == p.len() {
            j == t.len()
        } else if p[i] == '*' && p.get(i + 1) == Some(&'*') {
            // `**`, and `**/` also matching no folder at all.
            let next = i + 2;
            let skip_slash = p.get(next) == Some(&'/');
            (skip_slash && go(p, t, next + 1, j, paths, memo))
                || (j..=t.len()).any(|k| go(p, t, next, k, paths, memo))
        } else if p[i] == '*' {
            let mut k = j;
            loop {
                if go(p, t, i + 1, k, paths, memo) {
                    break true;
                }
                if k == t.len() || (paths && t[k] == '/') {
                    break false;
                }
                k += 1;
            }
        } else if p[i] == '?' {
            j < t.len() && !(paths && t[j] == '/') && go(p, t, i + 1, j + 1, paths, memo)
        } else {
            j < t.len() && p[i] == t[j] && go(p, t, i + 1, j + 1, paths, memo)
        };
        memo[i][j] = Some(result);
        result
    }
    go(&p, &t, 0, 0, paths, &mut memo)
}

/// Whether an earlier "Allow for this session" covers a new request of the same kind: any repo read, the same
/// file for a repo write, the same command line in the same folder for a command.
pub fn grant_covers(grant: &PermissionRecord, kind: PermissionKind, action: &Action) -> bool {
    if grant.kind != kind || !kind.session_grant_offered() {
        return false;
    }
    match kind {
        PermissionKind::ReadRepo => true,
        PermissionKind::WriteRepo | PermissionKind::RunCommand => &grant.action == action,
        _ => false,
    }
}

/// What the policy says before anyone is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    Decided {
        answer: Answer,
        source: DecisionSource,
    },
    /// Ask the person, offering these answers.
    Ask { offered: Vec<Answer> },
}

/// Decides `action` (already classified) without asking anyone, in this order: a deny rule; skip-all; the
/// engine's auto mode; an allow setting or rule; an earlier "Allow for this session" in `events`; otherwise ask.
pub fn evaluate(
    settings: &PermissionSettings,
    classified: &Classified,
    action: &Action,
    engine: Option<&str>,
    engine_verdict: Option<bool>,
    events: &[SessionEvent],
) -> Verdict {
    let decided = |answer, source| Verdict::Decided { answer, source };
    if let Some(rule) = settings
        .rules
        .iter()
        .find(|rule| rule.effect == Effect::Deny && rule.matches(classified))
    {
        return decided(
            Answer::Deny,
            DecisionSource::Rule {
                rule: rule.to_string(),
            },
        );
    }
    if settings.skip_all {
        return decided(Answer::AllowOnce, DecisionSource::SkipAll);
    }
    if let (Some(engine), Some(verdict)) = (engine, engine_verdict)
        && settings.engine_auto.contains(engine)
    {
        return decided(
            if verdict {
                Answer::AllowOnce
            } else {
                Answer::Deny
            },
            DecisionSource::EngineAuto {
                engine: engine.to_owned(),
            },
        );
    }
    let kind = classified.kind;
    let setting = match kind {
        PermissionKind::ReadRepo => Some(("read_repo", settings.read_repo)),
        PermissionKind::WriteRepo => Some(("write_files", settings.write_files)),
        PermissionKind::RunCommand => Some(("run_commands", settings.run_commands)),
        _ => None,
    };
    if let Some((name, Setting::Allow)) = setting {
        return decided(
            Answer::AllowOnce,
            DecisionSource::Rule {
                rule: format!("{name} = \"allow\""),
            },
        );
    }
    if setting.is_some()
        && let Some(rule) = settings
            .rules
            .iter()
            .find(|rule| rule.effect == Effect::Allow && rule.matches(classified))
    {
        return decided(
            Answer::AllowOnce,
            DecisionSource::Rule {
                rule: rule.to_string(),
            },
        );
    }
    if let Some(grant) = events.iter().find_map(|event| match &event.body {
        EventBody::Permission(record)
            if record.answer == Answer::AllowForSession
                && record.source == DecisionSource::Person
                && grant_covers(record, kind, action) =>
        {
            Some(event.seq)
        }
        _ => None,
    }) {
        return decided(Answer::AllowOnce, DecisionSource::SessionGrant { grant });
    }
    Verdict::Ask {
        offered: if kind.session_grant_offered() {
            vec![Answer::AllowOnce, Answer::AllowForSession, Answer::Deny]
        } else {
            vec![Answer::AllowOnce, Answer::Deny]
        },
    }
}
