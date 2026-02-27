use serde_json::{Value, json};
use sysinfo::System;

// All names must be lowercase to match the lowercased process name
const DEFAULT_PROCESS_NAMES: &[&str] = &[
    "cmd.exe",
    "cmd",
    "conhost.exe",
    "powershell.exe",
    "pwsh",
    "sh",
    "bash",
    "zsh",
    "node.exe",
    "node",
];

pub fn info() -> Value {
    json!({
        "name": "process_list",
        "description": "List running processes (cmd, powershell, sh, bash, zsh, node, etc.).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "filter": { "type": "string", "description": "Filter by process name" },
                "all": { "type": "boolean", "description": "List all running processes. When false (default), only lists common shell and runtime processes (cmd, powershell, bash, node, etc.)." }
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
    let show_all = arguments
        .get("all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let sys = System::new_all();

    let mut processes = Vec::new();
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();

        let matches_filter = if !filter.is_empty() {
            name.contains(&filter)
        } else if show_all {
            true
        } else {
            DEFAULT_PROCESS_NAMES.contains(&name.as_str())
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
                "start_time_unix": process.start_time(),
                "CommandLine": cmd_line,
                "MemoryKB": process.memory() / 1024,
                "Status": format!("{:?}", process.status()),
            }));
        }
    }

    let json_text = serde_json::to_string(&processes).map_err(|e| e.to_string())?;
    Ok(json!([{ "type": "text", "text": json_text }]))
}
