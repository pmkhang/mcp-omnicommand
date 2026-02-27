use serde_json::{Value, json};
use sysinfo::System;

pub fn info() -> Value {
    json!({
        "name": "system_info",
        "description": "Get basic system info: OS, architecture, memory, username, and shell.",
        "inputSchema": {
            "type": "object",
            "properties": {}
        }
    })
}

pub async fn run(_arguments: &Value) -> Result<Value, String> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let os_name = System::name().unwrap_or_else(|| "Unknown OS".to_string());
    let os_version = System::os_version().unwrap_or_default();
    let os_full = format!("{} {}", os_name, os_version);

    let mem_total = (sys.total_memory() as f64) / 1024.0 / 1024.0 / 1024.0;
    let mem_free = (sys.free_memory() as f64) / 1024.0 / 1024.0 / 1024.0;

    let arch = std::env::consts::ARCH;
    let username = if cfg!(windows) {
        std::env::var("USERNAME").unwrap_or_else(|_| "Unknown".to_string())
    } else {
        std::env::var("USER").unwrap_or_else(|_| "Unknown".to_string())
    };

    let shell = if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    };

    let p = json!({
        "OS": os_full.trim(),
        "Arch": arch,
        "User": username,
        "Shell": shell,
        "TotalMemoryGB": format!("{:.1}", mem_total).parse::<f64>().unwrap_or(0.0),
        "FreeMemoryGB": format!("{:.1}", mem_free).parse::<f64>().unwrap_or(0.0)
    });

    Ok(json!([{ "type": "text", "text": serde_json::to_string(&p).unwrap_or_default() }]))
}
