//! Projection for the opt-in fixture recorder. Never save raw stdout, stderr, auth output or init metadata.
use serde_json::{Value, json};
/// Retains only fields consumed by the decoder. The recorder's prompt is fixed TEST DATA, with no attachments.
pub fn record(value: &Value) -> Option<Value> {
    let kind = value["type"].as_str()?;
    Some(match kind {
        "system" if value["subtype"] == "init" => {
            json!({"type":"system","subtype":"init","mcp_servers":value["mcp_servers"].as_array().map(|items|items.iter().map(|v|json!({"name":v["name"],"status":v["status"]})).collect::<Vec<_>>()).unwrap_or_default()})
        }
        "assistant" => {
            if let Some(error) = value.get("error") {
                json!({"type":"assistant","error":error.as_str().filter(|e|matches!(*e,"authentication_failed"|"rate_limit"|"unknown")),"message":{"content":[]}})
            } else {
                json!({"type":"assistant","message":{"content":value["message"]["content"].as_array()?.iter().filter_map(|p|match p["type"].as_str()? {"text"=>Some(json!({"type":"text","text":p["text"]})),"tool_use"=>Some(json!({"type":"tool_use","id":p["id"],"name":p["name"],"input":p["input"]})),_=>None}).collect::<Vec<_>>()}})
            }
        }
        "stream_event" if value["event"]["delta"]["type"] == "text_delta" => {
            json!({"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":value["event"]["delta"]["text"]}}})
        }
        "result" => {
            let mut out =
                json!({"type":"result","subtype":value["subtype"],"is_error":value["is_error"]});
            if value["is_error"] == false {
                out["result"] = value["result"].clone();
            }
            if let Some(usage) = value.get("usage") {
                out["usage"] = json!({});
                for key in [
                    "input_tokens",
                    "output_tokens",
                    "cache_read_input_tokens",
                    "cache_creation_input_tokens",
                ] {
                    if let Some(n) = usage[key].as_u64() {
                        out["usage"][key] = json!(n);
                    }
                }
            }
            out
        }
        "rate_limit_event" => {
            json!({"type":"rate_limit_event","rate_limit_info":{"status":value["rate_limit_info"]["status"],"resetsAt":value["rate_limit_info"]["resetsAt"].as_i64()}})
        }
        "error" => json!({"type":"error"}),
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_and_identity_are_never_recorded() {
        let canary = "TEST DATA canary-native-token";
        for value in [
            json!({"type":"system","subtype":"init","cwd":canary,"apiKeySource":canary,"session_id":canary}),
            json!({"type":"assistant","error":"authentication_failed","message":{"content":[{"type":"text","text":canary}]}}),
            json!({"type":"result","subtype":"error_during_execution","is_error":true,"errors":[canary],"result":canary,"usage":{"accessToken":canary}}),
        ] {
            assert!(!record(&value).unwrap().to_string().contains(canary));
        }
    }
}
