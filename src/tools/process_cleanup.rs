use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;

pub fn info() -> Value {
    json!({
        "name": "process_cleanup",
        "description": "Find and kill hanging/orphaned shell processes (cmd, sh, bash, zsh, etc.).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "maxAgeSeconds": { "type": "number" },
                "dryRun": { "type": "boolean" },
                "includeNode": { "type": "boolean" }
            }
        }
    })
}

pub async fn run(arguments: &Value) -> Result<Value, String> {
    let max_age_seconds = arguments
        .get("maxAgeSeconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);

    let include_node = arguments
        .get("includeNode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut sys = System::new_all();
    sys.refresh_all();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut results = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().into_owned();
        let lower_name = name.to_lowercase();

        let is_shell = if cfg!(windows) {
            lower_name == "cmd.exe" || lower_name == "conhost.exe" || lower_name == "powershell.exe"
        } else {
            lower_name == "sh" || lower_name == "bash" || lower_name == "zsh"
        };

        let is_target = is_shell || (include_node && lower_name.contains("node"));

        if is_target {
            let start_time = process.start_time();
            if start_time == 0 {
                continue;
            }
            if now > start_time {
                let age = now - start_time;
                if age >= max_age_seconds {
                    let status = if process.kill() { "Killed" } else { "Failed" };
                    results.push(json!({
                        "PID": pid.as_u32(),
                        "Name": name,
                        "Age": age,
                        "Status": status
                    }));
                }
            }
        }
    }

    Ok(json!([{ "type": "text", "text": serde_json::to_string(&results).unwrap_or_default() }]))
}
