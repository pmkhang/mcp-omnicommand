use crate::SERVER_VERSION;
use crate::tools;
use serde_json::{Value, json};
use std::io::{Write, stdout};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};

pub async fn run_mcp_server() {
    let stdin_stream = stdin();
    let mut reader = BufReader::new(stdin_stream).lines();
    let mut default_cwd: Option<String> = None;

    while let Ok(Some(line)) = reader.next_line().await {
        if let Ok(request) = serde_json::from_str::<Value>(&line) {
            handle_rpc_message(&request, &mut default_cwd).await;
        }
    }
}

async fn handle_rpc_message(msg: &Value, default_cwd: &mut Option<String>) {
    let jsonrpc = msg.get("jsonrpc").and_then(Value::as_str).unwrap_or("");
    if jsonrpc != "2.0" {
        return;
    }

    let id = msg.get("id");
    let method = msg.get("method").and_then(Value::as_str);

    // Notifications (no id) are intentionally ignored —
    // e.g. notifications/initialized, notifications/cancelled
    let Some(id_val) = id else {
        return;
    };

    if let Some(method_name) = method {
        match method_name {
            "initialize" => handle_initialize(id_val, msg, default_cwd),
            "tools/list" => handle_tools_list(id_val),
            "tools/call" => handle_tools_call(id_val, msg, default_cwd.as_deref()).await,
            _ => {
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "error": { "code": -32601, "message": "Method not found" }
                });
                send_response(&error_response);
            }
        }
    }
}

fn handle_initialize(id: &Value, msg: &Value, default_cwd: &mut Option<String>) {
    if let Some(p) = msg.get("params") {
        let mut path = p.get("rootPath").and_then(Value::as_str).map(String::from);

        if path.is_none()
            && let Some(uri) = p.get("rootUri").and_then(Value::as_str)
        {
            let stripped = uri
                .strip_prefix("file:///")
                .or_else(|| uri.strip_prefix("file://"));
            if let Some(s) = stripped {
                #[cfg(windows)]
                {
                    path = Some(s.replace('/', "\\"));
                }
                #[cfg(not(windows))]
                {
                    path = Some(s.to_string());
                }
            }
        }

        if let Some(p_val) = path {
            *default_cwd = Some(p_val);
        }
    }

    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "omni", "version": SERVER_VERSION }
        }
    });
    send_response(&response);
}

fn handle_tools_list(id: &Value) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "tools": tools::get_tools() }
    });
    send_response(&response);
}

async fn handle_tools_call(id: &Value, msg: &Value, default_cwd: Option<&str>) {
    let empty_json = json!({});
    let params = msg.get("params").unwrap_or(&empty_json);
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").unwrap_or(&empty_json);

    match tools::handle_tool_call(name, arguments, default_cwd).await {
        Ok(content) => {
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "content": content }
            });
            send_response(&response);
        }
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32603, "message": e }
            });
            send_response(&error_response);
        }
    }
}

fn send_response(resp: &Value) {
    if let Ok(json_str) = serde_json::to_string(resp) {
        println!("{json_str}");
        let _ = stdout().flush();
    }
}
