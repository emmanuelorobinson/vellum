//! Prints both sync envelope schemas; no application runtime or vendor adapter is needed.
fn main() {
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"client":vellum_types::sync::client_schema(), "server":vellum_types::sync::server_schema()})).expect("schema serializes"));
}
