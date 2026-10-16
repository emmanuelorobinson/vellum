//! Keeping a turn inside its model's context window. Nothing is dropped silently: a long tool result is cut with
//! a marker that says how much was left out, an old result or picture is replaced by a marker that names it, and
//! older rounds are replaced by a summary that is labelled as one.
use serde_json::{Value, json};
use vellum_core::ports::{ChatMessage, ChatPart, ChatRole, ModelTool, Truncation};

/// When the model does not say, assume a window this size (tokens). Never unlimited.
pub const UNKNOWN_WINDOW: u64 = 128_000;
/// Compaction starts when the estimate passes this share of the window left after the reply.
const THRESHOLD: f64 = 0.8;
/// Rounds (assistant messages and their results) kept verbatim at the end of the conversation.
pub const KEEP_ROUNDS: usize = 2;
/// A rough guess at what one picture costs.
const PICTURE_TOKENS: u64 = 1_500;
/// How much of each message a summary request quotes.
const QUOTE_CHARS: usize = 2_000;

fn chars_tokens(chars: usize) -> u64 {
    (chars as u64).div_ceil(4)
}

/// A deliberately simple estimate (four characters a token): the point is to stay well clear of the limit.
pub fn estimate(system: &str, tools: &[ModelTool], messages: &[ChatMessage]) -> u64 {
    let tools: usize = tools
        .iter()
        .map(|t| t.name.len() + t.description.len() + t.parameters.to_string().len())
        .sum();
    let parts: u64 = messages
        .iter()
        .flat_map(|m| &m.content)
        .map(|part| match part {
            ChatPart::Text(text) => chars_tokens(text.len()),
            ChatPart::Image { .. } => PICTURE_TOKENS,
            ChatPart::ToolCall(call) => {
                chars_tokens(call.name.len() + call.arguments.to_string().len())
            }
            ChatPart::ToolResult { result, .. } => chars_tokens(result.to_string().len()),
            ChatPart::Continuation(value) => chars_tokens(value.to_string().len()),
        })
        .sum();
    chars_tokens(system.len() + tools) + parts
}

/// The token budget for the request's input, and the point where compaction starts.
pub fn budget(window: Option<u64>, max_output: u32) -> (u64, u64) {
    let budget = window
        .unwrap_or(UNKNOWN_WINDOW)
        .saturating_sub(u64::from(max_output));
    (budget, (budget as f64 * THRESHOLD) as u64)
}

/// Cuts a tool result longer than `max` characters of JSON, saying so.
pub fn truncate(value: Value, max: usize) -> (Value, Option<Truncation>) {
    let text = value.to_string();
    let total = text.chars().count();
    if total <= max {
        return (value, None);
    }
    let partial: String = text.chars().take(max).collect();
    let marker = format!(
        "This result was {total} characters; only the first {max} are shown below. Ask for less if you need the \
         rest: a smaller scope, fewer nodes, or problems_only."
    );
    (
        json!({"truncated": marker, "partial": partial}),
        Some(Truncation {
            total_chars: total,
            shown_chars: max,
        }),
    )
}

/// Where the kept tail starts: the assistant message that opens the last [`KEEP_ROUNDS`] rounds. `None` when there
/// is nothing older to cut.
pub fn cut_point(messages: &[ChatMessage]) -> Option<usize> {
    let rounds: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| m.role == ChatRole::Assistant)
        .map(|(at, _)| at)
        .collect();
    let at = *rounds.get(rounds.len().checked_sub(KEEP_ROUNDS)?)?;
    (at > 0).then_some(at)
}

const REMOVED_PICTURE: &str = "[An earlier picture was removed to save context.]";

/// Replaces tool results and pictures before `cut` with markers. Returns how many it replaced.
pub fn prune(messages: &mut [ChatMessage], cut: usize) -> usize {
    let mut pruned = 0;
    for message in &mut messages[..cut] {
        for part in &mut message.content {
            match part {
                ChatPart::ToolResult { name, result, .. } if result.get("removed").is_none() => {
                    let size = result.to_string().chars().count();
                    *result = json!({"removed": format!(
                        "An earlier result of {name} ({size} characters) was removed to save context. Call {name} again if you need it."
                    )});
                    pruned += 1;
                }
                ChatPart::Image { .. } => {
                    *part = ChatPart::Text(REMOVED_PICTURE.to_owned());
                    pruned += 1;
                }
                _ => {}
            }
        }
    }
    pruned
}

fn quote(text: &str) -> String {
    let count = text.chars().count();
    if count <= QUOTE_CHARS {
        text.to_owned()
    } else {
        let head: String = text.chars().take(QUOTE_CHARS).collect();
        format!("{head} [... {} more characters]", count - QUOTE_CHARS)
    }
}

/// The messages before a cut as plain text, for a summary request that offers no tools.
pub fn transcript(messages: &[ChatMessage]) -> String {
    let mut out = String::new();
    for message in messages {
        let who = match message.role {
            ChatRole::User => "User",
            ChatRole::Assistant => "Agent",
        };
        for part in &message.content {
            let line = match part {
                ChatPart::Text(text) => format!("{who}: {}", quote(text)),
                ChatPart::Image { .. } => format!("{who}: [a picture]"),
                ChatPart::ToolCall(call) => {
                    format!(
                        "Agent called {}: {}",
                        call.name,
                        quote(&call.arguments.to_string())
                    )
                }
                ChatPart::ToolResult {
                    name,
                    result,
                    failed,
                    ..
                } => format!(
                    "{name} {}: {}",
                    if *failed { "failed" } else { "returned" },
                    quote(&result.to_string())
                ),
                ChatPart::Continuation(_) => continue,
            };
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// The message that replaces the summarized rounds: the person's ask, then the summary, both labelled.
pub fn summary_message(ask: &str, summary: &str) -> ChatMessage {
    ChatMessage {
        role: ChatRole::User,
        content: vec![ChatPart::Text(format!(
            "The person asked:\n{ask}\n\nEarlier work in this conversation was replaced by this summary to save \
             context:\n{summary}\n\nContinue from here."
        ))],
    }
}
