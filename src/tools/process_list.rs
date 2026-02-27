use serde_json::{Value, json};
use sysinfo::System;

pub fn info() -> Value {
    json!({
        "name": "process_list",
        "description": "List running processes (cmd, powershell, sh, bash, zsh, node, etc.).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Filter by process name" }
            }
        }
    })
}

pub fn run(arguments: &Value) -> Result<Value, String> {
    let filter = arguments
        .get("filter")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();

    let mut sys = System::new_all();
    sys.refresh_all();

    let mut processes = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();

        let matches_filter = if filter.is_empty() {
            name == "cmd.exe"
                || name == "cmd"
                || name == "conhost.exe"
                || name == "powershell.exe"
                || name == "pwsh"
                || name == "sh"
                || name == "bash"
                || name == "zsh"
                || name == "node.exe"
                || name == "node"
        } else {
            name.contains(&filter)
        };

        if matches_filter {
            let cmd_line = process
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect::<Vec<String>>()
                .join(" ");

            processes.push(json!({
                "ProcessId": pid.as_u32(),
                "Name": process.name().to_string_lossy(),
                "CreationDate": process.start_time(),
                "CommandLine": cmd_line,
            }));
        }
    }

    let json_text = serde_json::to_string(&processes).map_err(|e| e.to_string())?;
    Ok(json!([{ "type": "text", "text": json_text }]))
}
