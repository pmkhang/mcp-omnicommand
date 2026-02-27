use serde_json::{Value, json};
#[cfg(windows)]
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, Process, System};

#[cfg_attr(not(windows), allow(unused_variables))]
fn kill_process(process: &Process, pid_u32: u32, force: bool) -> bool {
    if force {
        #[cfg(windows)]
        {
            Command::new("taskkill")
                .args(["/F", "/PID", &pid_u32.to_string()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            process.kill_with(sysinfo::Signal::Kill).unwrap_or(false)
        }
    } else {
        process.kill()
    }
}

pub fn info() -> Value {
    json!({
        "name": "process_kill",
        "description": "Force kill: SIGKILL on Linux/macOS, taskkill /F on Windows. Default false sends SIGTERM.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pid": { "type": "number", "description": "PID of the process to kill" },
                "name": { "type": "string", "description": "Name of the process to kill (kills all matches)" },
                "cleanup": { "type": "boolean", "description": "Enable cleanup mode for orphaned shell processes" },
                "maxAgeSeconds": { "type": "number", "description": "Max age in seconds for cleanup mode (default: 30)" },
                "includeNode": { "type": "boolean", "description": "Include Node.js processes in cleanup mode" },
                "force": { "type": "boolean", "description": "Force kill (Windows: /F)" }
            }
        }
    })
}

pub fn run(arguments: &Value) -> Result<Value, String> {
    let pid_arg = arguments.get("pid").and_then(Value::as_u64);
    let name_arg = arguments.get("name").and_then(Value::as_str);
    let cleanup_arg = arguments
        .get("cleanup")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let force = arguments
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let sys = System::new_all();

    let mut killed_count = 0;
    let mut details = Vec::new();

    if cleanup_arg {
        let max_age_seconds = arguments
            .get("maxAgeSeconds")
            .and_then(Value::as_u64)
            .unwrap_or(30);
        let include_node = arguments
            .get("includeNode")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().into_owned();
            let lower_name = name.to_lowercase();

            let is_shell = if cfg!(windows) {
                lower_name == "cmd.exe"
                    || lower_name == "conhost.exe"
                    || lower_name == "powershell.exe"
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
                        if kill_process(process, pid.as_u32(), force) {
                            killed_count += 1;
                            details.push(format!(
                                "Cleaned up PID {} ({}, Age: {}s)",
                                pid.as_u32(),
                                name,
                                age
                            ));
                        } else {
                            details.push(format!(
                                "Failed to kill PID {} ({}, Age: {}s)",
                                pid.as_u32(),
                                name,
                                age
                            ));
                        }
                    }
                }
            }
        }
    } else if let Some(p) = pid_arg {
        let pid = Pid::from(usize::try_from(p).unwrap_or(usize::MAX));
        if let Some(process) = sys.process(pid) {
            let proc_name = process.name().to_string_lossy().into_owned();
            if kill_process(process, pid.as_u32(), force) {
                killed_count += 1;
                details.push(format!("Killed PID {p} ({proc_name})"));
            } else {
                return Err(format!("Failed to kill PID {p}"));
            }
        } else {
            return Err(format!("Process with PID {p} not found"));
        }
    } else if let Some(n) = name_arg {
        let target_name = n.to_lowercase();
        for (pid, process) in sys.processes() {
            let proc_name = process.name().to_string_lossy().to_lowercase();
            if proc_name.contains(&target_name) {
                let actual_name = process.name().to_string_lossy().into_owned();
                if kill_process(process, pid.as_u32(), force) {
                    killed_count += 1;
                    details.push(format!("Killed PID {} ({actual_name})", pid.as_u32()));
                }
            }
        }
    } else {
        return Err("Provide 'pid', 'name', or set 'cleanup' to true".to_string());
    }

    if killed_count == 0 && details.is_empty() {
        return Ok(json!([{ "type": "text", "text": "No matching processes found." }]));
    }

    Ok(
        json!([{ "type": "text", "text": format!("Killed {killed_count} process(es):\n{}", details.join("\n")) }]),
    )
}
