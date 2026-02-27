mod process;
mod tools;

use serde_json::{Value, json};
use std::env;
use std::process::exit;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    // Nếu có đối số dòng lệnh, chạy ở chế độ CLI trực tiếp
    if args.len() >= 2 {
        run_standalone_cli(&args).await;
        return;
    }

    // Luồng đọc từ stdin (MCP Server mode)
    let stdin_stream = stdin();
    let mut reader = BufReader::new(stdin_stream).lines();
    let mut default_cwd: Option<String> = None;

    while let Ok(Some(line)) = reader.next_line().await {
        if let Ok(request) = serde_json::from_str::<Value>(&line) {
            handle_rpc_message(&request, &mut default_cwd).await;
        }
    }
}

async fn run_standalone_cli(args: &[String]) {
    let tool_name = &args[1];

    // Xử lý cờ phiên bản
    if tool_name == "--version" || tool_name == "-v" {
        println!("Omnicommand version 1.0.0");
        return;
    }

    let mut arguments = json!({});

    // Phân giải các đối số dạng --key value
    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            let key = arg.trim_start_matches("--");
            if i + 1 < args.len() {
                let val = &args[i + 1];
                // Thử parse thành số hoặc boolean nếu có thể
                if let Ok(num) = val.parse::<u64>() {
                    arguments[key] = json!(num);
                } else if val == "true" {
                    arguments[key] = json!(true);
                } else if val == "false" {
                    arguments[key] = json!(false);
                } else {
                    arguments[key] = json!(val);
                }
                i += 2;
            } else {
                arguments[key] = json!(true);
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    match tools::handle_tool_call(tool_name, &arguments, None).await {
        Ok(res) => {
            if let Ok(pretty) = serde_json::to_string_pretty(&res) {
                println!("{pretty}");
            } else {
                eprintln!("Failed to serialize response");
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            exit(1);
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

    // Nếu là notification (id = null or missing), bỏ qua rpc đơn giản
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
            "serverInfo": { "name": "Omnicommand", "version": "1.0.0" }
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
    }
}
