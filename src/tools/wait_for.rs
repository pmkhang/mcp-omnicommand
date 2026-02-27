use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::net::TcpStream;
use tokio::time::sleep;

pub fn info() -> Value {
    json!({
        "name": "wait_for",
        "description": "Wait for a condition (port, file, or process) to be met.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "strategy": { "type": "string", "enum": ["port", "file", "process"], "description": "What to wait for" },
                "target": { "type": "string", "description": "The port address (e.g. '127.0.0.1:8080'), file path, or process name/PID" },
                "timeout": { "type": "number", "description": "Timeout in milliseconds", "default": 30000 },
                "interval": { "type": "number", "description": "Polling interval in milliseconds", "default": 500 }
            },
            "required": ["strategy", "target"]
        }
    })
}

pub async fn run(arguments: &Value) -> Result<Value, String> {
    let strategy = arguments
        .get("strategy")
        .and_then(Value::as_str)
        .ok_or("Strategy is required")?;
    let target = arguments
        .get("target")
        .and_then(Value::as_str)
        .ok_or("Target is required")?;
    let timeout_ms = arguments
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(30000);
    let interval_ms = arguments
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(500);

    let start_time = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let interval = Duration::from_millis(interval_ms);

    let mut sys = System::new_all();

    while start_time.elapsed() < timeout {
        let success = match strategy {
            "port" => TcpStream::connect(target).await.is_ok(),
            "file" => Path::new(target).exists(),
            "process" => {
                sys.refresh_processes(ProcessesToUpdate::All, true);
                if let Ok(pid) = target.parse::<usize>() {
                    sys.process(Pid::from(pid)).is_none()
                } else {
                    !sys.processes()
                        .values()
                        .any(|p| p.name().to_string_lossy().contains(target))
                }
            }
            _ => return Err(format!("Unknown strategy: {strategy}")),
        };

        if success {
            return Ok(json!([{
                "type": "text",
                "text": serde_json::to_string(&json!({
                    "status": "success",
                    "elapsed_ms": u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX)
                })).unwrap_or_default()
            }]));
        }

        sleep(interval).await;
    }

    Err(format!(
        "Timeout waiting for {strategy}: {target} after {timeout_ms}ms"
    ))
}
