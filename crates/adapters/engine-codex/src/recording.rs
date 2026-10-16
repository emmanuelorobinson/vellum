//! Allowlisted projection for the fixed TEST DATA recorder. Never persist configuration or auth replies.
use serde_json::{Value, json};
pub fn record(v: &Value) -> Option<Value> {
    if let Some(kind) = v["type"].as_str() {
        return Some(match kind {
            "thread.started" => json!({"type":kind,"thread_id":"TEST-thread"}),
            "turn.started" => json!({"type":kind}),
            "item.completed" if matches!(v["item"]["type"].as_str(), Some("agent_message")) => {
                json!({"type":kind,"item":{"type":"agent_message","id":safe_item_id(&v["item"]["id"]),"text":v["item"]["text"]}})
            }
            "turn.completed" => {
                let mut out = json!({"type":kind,"usage":{}});
                for key in [
                    "input_tokens",
                    "cached_input_tokens",
                    "output_tokens",
                    "reasoning_output_tokens",
                    "cache_write_input_tokens",
                ] {
                    if let Some(n) = v["usage"][key].as_u64() {
                        out["usage"][key] = json!(n);
                    }
                }
                out
            }
            "turn.failed" | "error" => {
                json!({"type":kind,"error":{"codexErrorInfo":safe_error(v.get("error").unwrap_or(v))}})
            }
            _ => return None,
        });
    }
    if v["method"] == "item/tool/call" && v["params"]["tool"] == "get_app_state" {
        return Some(
            json!({"id":"TEST-call","method":"item/tool/call","params":{"threadId":"TEST-thread","turnId":"TEST-turn","callId":"TEST-call","tool":"get_app_state","arguments":{}}}),
        );
    }
    if let Some(id) = v["id"].as_u64() {
        if v.get("method").is_some() {
            return None;
        }
        return Some(if v.get("error").is_some() {
            json!({"id":id,"error":{"code":v["error"]["code"].as_i64()}})
        } else {
            match id {
                1 => json!({"id":1,"result":{}}),
                2 => json!({"id":2,"result":{"config":{}}}),
                3 => json!({"id":3,"result":{"thread":{"id":"TEST-thread"}}}),
                4 => json!({"id":4,"result":{"turn":{"id":"TEST-turn"}}}),
                _ => return None,
            }
        });
    }
    let method = v["method"].as_str()?;
    let p = &v["params"];
    let mut out = json!({"method":method,"params":{"threadId":"TEST-thread","turnId":"TEST-turn"}});
    match method {
        "mcpServer/startupStatus/updated" if p["name"] == "vellum" => {
            out["params"] =
                json!({"name":"vellum","status":p["status"],"failureReason":p["failureReason"]});
        }
        "item/agentMessage/delta" => {
            out["params"]["delta"] = p["delta"].clone();
            out["params"]["itemId"] = json!("TEST-message");
        }
        "item/completed" if p["item"]["type"] == "agentMessage" => {
            out["params"]["item"] =
                json!({"type":"agentMessage","id":"TEST-message","text":p["item"]["text"]})
        }
        "thread/tokenUsage/updated" => {
            out["params"]["tokenUsage"] = json!({"total":{}});
            for key in [
                "inputTokens",
                "cachedInputTokens",
                "cacheWriteInputTokens",
                "outputTokens",
                "reasoningOutputTokens",
            ] {
                if let Some(n) = p["tokenUsage"]["total"][key].as_u64() {
                    out["params"]["tokenUsage"]["total"][key] = json!(n);
                }
            }
        }
        "turn/completed" => {
            out["params"]["turn"] = json!({"id":"TEST-turn","status":p["turn"]["status"]});
            if let Some(e) = p["turn"].get("error").filter(|e| !e.is_null()) {
                out["params"]["turn"]["error"] = json!({"codexErrorInfo":safe_error(e)});
            }
        }
        _ => return None,
    }
    Some(out)
}
fn safe_error(e: &Value) -> Value {
    match crate::protocol::error_kind(e) {
        Some(
            s @ ("unauthorized"
            | "usageLimitExceeded"
            | "rateLimitExceeded"
            | "contextWindowExceeded"),
        ) => json!(s),
        _ => json!("other"),
    }
}

fn safe_item_id(v: &Value) -> &str {
    v.as_str()
        .filter(|s| {
            s.len() < 40
                && s.strip_prefix("item_")
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .unwrap_or("TEST-message")
}
