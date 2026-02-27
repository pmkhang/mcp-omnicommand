use crate::SERVER_VERSION;
use crate::tools;
use serde_json::{Value, json};
use std::process::exit;

pub async fn run_standalone_cli(args: &[String]) {
    let tool_name = &args[1];

    // Xử lý cờ phiên bản
    if tool_name == "--version" || tool_name == "-v" {
        println!("Omnicommand version {SERVER_VERSION}");
        return;
    }

    // Xử lý cờ help
    if tool_name == "--help" || tool_name == "-h" {
        println!("Omnicommand CLI");
        println!("Usage: omnicommand <tool_name> [--key value] [--key=value]\n");
        println!("Available tools:");
        if let Some(tools_arr) = tools::get_tools().as_array() {
            for tool in tools_arr {
                if let Some(name) = tool.get("name").and_then(Value::as_str) {
                    let desc = tool
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    println!("  {name:<25} {desc}");
                }
            }
        }
        return;
    }

    let mut arguments = json!({});

    // Phân giải các đối số dạng --key value
    let mut i = 2;
    while i < args.len() {
        let arg = &args[i];
        if arg.starts_with("--") {
            // Handle both --key value and --key=value formats
            let (key, val_opt) = if arg.contains('=') {
                let parts: Vec<&str> = arg.splitn(2, '=').collect();
                (
                    parts[0].trim_start_matches("--"),
                    Some(parts[1].to_string()),
                )
            } else {
                (arg.trim_start_matches("--"), None)
            };

            let val = if let Some(v) = val_opt {
                i += 1;
                v
            } else if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                i += 2;
                args[i - 1].clone()
            } else {
                arguments[key] = json!(true);
                i += 1;
                continue;
            };

            if let Ok(num) = val.parse::<u64>() {
                arguments[key] = json!(num);
            } else if val == "true" {
                arguments[key] = json!(true);
            } else if val == "false" {
                arguments[key] = json!(false);
            } else {
                arguments[key] = json!(val);
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
