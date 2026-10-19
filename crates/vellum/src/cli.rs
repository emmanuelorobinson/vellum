//! The CLI front end over the tool registry (ARCHITECTURE §3.4, §7.8): `vellum tool <name> --field value ...`.
//!
//! The subcommands and their flags are generated from each tool's input schema, so a tool added to the registry
//! is on the command line with no code here. A call goes to the daemon like any other client's, which is what
//! lets it reach a file an editor holds; the command prints the tool's result as JSON.

pub mod commands;
pub mod git;
pub mod prompt;

use std::io;
use std::path::Path;
use std::process::ExitCode;

use clap::builder::PossibleValuesParser;
use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use serde_json::{Map, Value, json};
use vellum_ipc_local::RunDir;

use crate::tools::{Registry, Tool};

/// The first sentence of a description: enough for a list of tools.
fn summary(text: &str) -> String {
    match text.find(". ") {
        Some(end) => text[..=end].to_owned(),
        None => text.to_owned(),
    }
}

/// The kind of flag a property is: a list of strings is its own kind (repeated or comma-separated); any other
/// list, like an object, is written as JSON.
fn property_type(schema: &Value) -> &str {
    match schema.get("type").and_then(Value::as_str).unwrap_or("") {
        "array" if schema.pointer("/items/type").and_then(Value::as_str) == Some("string") => {
            "array"
        }
        "array" => "json",
        other => other,
    }
}

fn flag(name: &str, schema: &Value, required: bool) -> Arg {
    let long = name.replace('_', "-");
    let mut arg = Arg::new(name.to_owned()).long(long).required(required);
    if let Some(help) = schema.get("description").and_then(Value::as_str) {
        arg = arg.help(summary(help)).long_help(help.to_owned());
    }
    if let Some(words) = schema.get("enum").and_then(Value::as_array) {
        let words: Vec<String> = words
            .iter()
            .filter_map(|w| w.as_str().map(str::to_owned))
            .collect();
        return arg.value_parser(PossibleValuesParser::new(words));
    }
    match property_type(schema) {
        "boolean" => arg.action(ArgAction::SetTrue).required(false),
        "integer" => arg.value_parser(value_parser!(u64)),
        "number" => arg.value_parser(value_parser!(f64)),
        "string" => arg,
        "array" => arg.action(ArgAction::Append).value_delimiter(','),
        // Objects and anything else are written as JSON.
        _ => arg.value_name("JSON").value_parser(|text: &str| {
            serde_json::from_str::<Value>(text)
                .map(|_| text.to_owned())
                .map_err(|e| format!("not valid JSON: {e}"))
        }),
    }
}

fn subcommand(tool: &dyn Tool) -> Command {
    let schema = serde_json::to_value(tool.input_schema()).unwrap_or(Value::Null);
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut command = Command::new(tool.name().to_owned())
        .about(summary(tool.description()))
        .long_about(tool.description().to_owned());
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            command = command.arg(flag(name, property, required.contains(&name.as_str())));
        }
    }
    command
}

/// `tool <name> ...` for every tool in `registry`.
pub fn command(registry: &Registry) -> Command {
    let mut tool = Command::new("tool")
        .about("Run one of Vellum's tools, the same ones an MCP client sees")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .disable_help_subcommand(true)
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print the whole MCP result as JSON, including a failure"),
        )
        .arg(
            Arg::new("image-out")
                .long("image-out")
                .global(true)
                .value_name("FILE")
                .help("Write the picture a tool returns to this file"),
        );
    for t in registry.tools() {
        tool = tool.subcommand(subcommand(t));
    }
    tool
}

/// The tool's input from the flags that were given.
pub fn input(tool: &dyn Tool, matches: &ArgMatches) -> Result<Value, String> {
    let schema = serde_json::to_value(tool.input_schema()).unwrap_or(Value::Null);
    let mut out = Map::new();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(Value::Object(out));
    };
    for (name, property) in properties {
        if matches.value_source(name) != Some(ValueSource::CommandLine) {
            continue;
        }
        let value = if property.get("enum").is_some() {
            json!(matches.get_one::<String>(name))
        } else {
            match property_type(property) {
                "boolean" => json!(matches.get_flag(name)),
                "integer" => json!(matches.get_one::<u64>(name)),
                "number" => json!(matches.get_one::<f64>(name)),
                "string" => json!(matches.get_one::<String>(name)),
                "array" => json!(
                    matches
                        .get_many::<String>(name)
                        .map(Iterator::collect::<Vec<_>>)
                ),
                _ => {
                    let text = matches.get_one::<String>(name).ok_or("missing value")?;
                    serde_json::from_str(text).map_err(|e| format!("--{name}: {e}"))?
                }
            }
        };
        out.insert(name.clone(), value);
    }
    Ok(Value::Object(out))
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut buffer, mut bits) = (0u32, 0);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Runs `vellum tool <args>`.
pub fn run(dir: &RunDir, executable: &Path, args: Vec<String>) -> io::Result<ExitCode> {
    let json = args.iter().any(|a| a == "--json");
    let output = commands::Output { json };
    let registry = Registry::standard();
    let matches = match command(&registry)
        .try_get_matches_from(std::iter::once("tool".to_owned()).chain(args))
    {
        Ok(matches) => matches,
        Err(error) => {
            // Help goes to stdout with success; a usage error to stderr with code 2.
            if json {
                if error.exit_code() == 0 {
                    println!("{}", json!({"ok":true,"help":error.to_string()}));
                } else {
                    output.error("tool", &error.to_string());
                }
            } else {
                error.print()?;
            }
            return Ok(ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2)));
        }
    };
    let (name, sub) = matches.subcommand().expect("a subcommand is required");
    let tool = registry.get(name).expect("the subcommands are the tools");
    let arguments = match input(tool, sub) {
        Ok(arguments) => arguments,
        Err(message) => {
            output.error("tool", &message);
            return Ok(ExitCode::from(2));
        }
    };
    let replies = vellum_ipc_local::exchange(
        dir,
        executable,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "vellum-cli", "version": env!("CARGO_PKG_VERSION")},
            }}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": arguments}}),
        ],
    )?;
    let reply = replies
        .get(1)
        .ok_or_else(|| io::Error::other("the daemon sent no reply"))?;
    if let Some(error) = reply.get("error") {
        output.error(
            "tool",
            error["message"]
                .as_str()
                .unwrap_or("the daemon refused the call"),
        );
        return Ok(ExitCode::FAILURE);
    }
    let result = &reply["result"];
    let failed = result["isError"] == true;
    if let Some(file) = sub.get_one::<String>("image-out") {
        let data = result["content"]
            .as_array()
            .and_then(|c| c.iter().find(|c| c["type"] == "image"))
            .and_then(|c| c["data"].as_str());
        match data.and_then(decode_base64) {
            Some(bytes) => std::fs::write(file, bytes)?,
            None if failed => {}
            None => {
                eprintln!("vellum: {name} returned no picture to write to {file}.");
                return Ok(ExitCode::FAILURE);
            }
        }
    }
    if sub.get_flag("json") {
        println!("{result}");
    } else if failed {
        eprintln!(
            "vellum: {}",
            result["content"][0]["text"]
                .as_str()
                .unwrap_or("the tool failed")
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&result["structuredContent"]).unwrap_or_default()
        );
    }
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decodes_with_and_without_padding() {
        assert_eq!(decode_base64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(decode_base64("aGVsbG8").unwrap(), b"hello");
        assert_eq!(decode_base64("").unwrap(), b"");
        assert!(decode_base64("a*b").is_none());
        // The PNG signature, as the renderer prints it.
        assert_eq!(
            &decode_base64("iVBORw0KGgo=").unwrap()[..8],
            b"\x89PNG\r\n\x1a\n"
        );
    }

    #[test]
    fn every_registered_tool_is_a_subcommand_with_a_flag_for_each_field() {
        let registry = Registry::standard();
        let tool = command(&registry);
        for t in registry.tools() {
            let sub = tool
                .find_subcommand(t.name())
                .unwrap_or_else(|| panic!("{}", t.name()));
            let schema = serde_json::to_value(t.input_schema()).unwrap();
            for field in schema["properties"].as_object().unwrap().keys() {
                assert!(
                    sub.get_arguments().any(|a| a.get_id() == field.as_str()),
                    "{} has --{field}",
                    t.name()
                );
            }
        }
    }

    #[test]
    fn flags_become_the_inputs_the_tool_expects() {
        let registry = Registry::standard();
        let parse = |args: &[&str]| {
            let matches = command(&registry)
                .try_get_matches_from(std::iter::once("tool").chain(args.iter().copied()))
                .unwrap();
            let (name, sub) = matches.subcommand().unwrap();
            input(registry.get(name).unwrap(), sub).unwrap()
        };
        assert_eq!(
            parse(&["get_nodes", "--ids", "a,b", "--ids", "c", "--depth", "2"]),
            json!({"ids": ["a", "b", "c"], "depth": 2})
        );
        assert_eq!(
            parse(&["snapshot_layout", "--problems-only", "--max-nodes", "10"]),
            json!({"problems_only": true, "max_nodes": 10})
        );
        assert_eq!(
            parse(&[
                "find_space",
                "--width",
                "390",
                "--height",
                "844.5",
                "--near",
                r#"{"x":1,"y":2}"#
            ]),
            json!({"width": 390.0, "height": 844.5, "near": {"x": 1, "y": 2}})
        );
        assert_eq!(
            parse(&["query_nodes", "--type", "text"]),
            json!({"type": "text"})
        );
        assert_eq!(parse(&["get_app_state"]), json!({}));
        // A list of objects is written as JSON; a list of strings still splits on commas.
        assert_eq!(
            parse(&[
                "update_nodes",
                "--updates",
                r#"[{"id":"a","set":{"name":"b,c"}}]"#
            ]),
            json!({"updates": [{"id": "a", "set": {"name": "b,c"}}]})
        );
        assert_eq!(
            parse(&["delete_nodes", "--ids", "a,b"]),
            json!({"ids": ["a", "b"]})
        );
        assert_eq!(
            parse(&["execute", "--code", "return 1;", "--validate-only"]),
            json!({"code": "return 1;", "validate_only": true})
        );
    }
}
