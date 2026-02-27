use serde_json::{Value, json};
use sysinfo::{Pid, System};

pub fn info() -> Value {
    json!({
        "name": "process_kill",
        "description": "Kill a process by PID or name.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pid": { "type": "number", "description": "PID of the process to kill" },
                "name": { "type": "string", "description": "Name of the process to kill (kills all matches)" },
                "force": { "type": "boolean", "description": "Force kill (Windows: /F)" }
            }
        }
    })
}

pub async fn run(arguments: &Value) -> Result<Value, String> {
    let pid_arg = arguments.get("pid").and_then(|v| v.as_u64());
    let name_arg = arguments.get("name").and_then(|v| v.as_str());
    let _force = arguments
        .get("force")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut sys = System::new_all();
    sys.refresh_all();

    let mut killed_count = 0;
    let mut details = Vec::new();

    if let Some(p) = pid_arg {
        let pid = Pid::from(p as usize);
        if let Some(process) = sys.process(pid) {
            let proc_name = process.name().to_string_lossy().into_owned();
            if process.kill() {
                killed_count += 1;
                details.push(format!("Killed PID {} ({})", p, proc_name));
            } else {
                return Err(format!("Failed to kill PID {}", p));
            }
        } else {
            return Err(format!("Process with PID {} not found", p));
        }
    } else if let Some(n) = name_arg {
        let target_name = n.to_lowercase();
        for (pid, process) in sys.processes() {
            let proc_name = process.name().to_string_lossy().to_lowercase();
            if proc_name.contains(&target_name) || proc_name == target_name {
                let actual_name = process.name().to_string_lossy().into_owned();
                if process.kill() {
                    killed_count += 1;
                    details.push(format!("Killed PID {} ({})", pid.as_u32(), actual_name));
                }
            }
        }
    } else {
        return Err("Either 'pid' or 'name' must be provided".to_string());
    }

    if killed_count == 0 {
        return Ok(json!([{ "type": "text", "text": "No matching processes found to kill." }]));
    }

    Ok(
        json!([{ "type": "text", "text": format!("Successfully killed {} process(es):\n{}", killed_count, details.join("\n")) }]),
    )
}
