use serde_json::{Value, json};

const SUPPORTED_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
const DEFAULT_VERSION: &str = "2025-06-18";

pub fn initialize_result(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&requested) {
        requested
    } else {
        DEFAULT_VERSION
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "szpont",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}
