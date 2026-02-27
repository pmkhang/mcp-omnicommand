use crate::process::{CommandResponse, exec_cmd, exec_powershell};
use futures::future;
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;

pub fn info() -> Value {
    json!({
        "name": "run_command",
        "description": "Run single or multiple shell commands with automatic shell detection and custom shell support.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "A single command to run." },
                "commands": {
                    "type": "array",
                    "description": "Multiple commands to run sequentially or in parallel.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "command": { "type": "string" },
                            "cwd": { "type": "string" },
                            "shell": { "type": "string", "description": "Optional specific shell for this command." },
                            "background": { "type": "boolean", "description": "Override global background for this command." },
                            "logFile": { "type": "string", "description": "Override global logFile for this command." }
                        },
                        "required": ["command"]
                    }
                },
                "shell": {
                    "type": "string",
                    "description": "Optional shell to use (e.g., 'cmd', 'powershell', 'pwsh', 'bash', 'zsh', 'sh'). Defaults to system default."
                },
                "cwd": { "type": "string", "description": "Default working directory for all commands." },
                "timeout": { "type": "number", "description": "Timeout in ms per command. Defaults to 30000." },
                "continueOnError": { "type": "boolean", "description": "For multiple commands, whether to continue if one fails." },
                "runParallel": { "type": "boolean", "description": "Execute multiple commands in parallel. Note: continueOnError must be true when runParallel is true." },
                "background": { "type": "boolean", "description": "Run the command in the background." },
                "logFile": { "type": "string", "description": "File path to redirect stdout and stderr to." }
            }
        }
    })
}

async fn execute_with_shell_choice(
    command_str: &str,
    cwd_opt: Option<&str>,
    timeout_ms: u64,
    shell_opt: Option<&str>,
    background: bool,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    let shell = shell_opt.unwrap_or("");
    if shell == "powershell" || shell == "pwsh" || (cfg!(windows) && shell == "ps") {
        exec_powershell(command_str, cwd_opt, timeout_ms, background, log_file).await
    } else {
        exec_cmd(
            command_str,
            cwd_opt,
            timeout_ms,
            shell_opt,
            background,
            log_file,
        )
        .await
    }
}

struct BatchContext<'a> {
    global_cwd: Option<&'a str>,
    timeout: u64,
    global_shell: Option<&'a str>,
    background: bool,
    log_file: Option<&'a str>,
    run_parallel: bool,
    continue_on_error: bool,
}

pub async fn run(arguments: &Value, default_cwd: Option<&str>) -> Result<Value, String> {
    let timeout = arguments
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(30000);
    let run_parallel = arguments
        .get("runParallel")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let continue_on_error = arguments
        .get("continueOnError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let global_shell = arguments.get("shell").and_then(Value::as_str);
    let global_cwd = arguments.get("cwd").and_then(Value::as_str).or(default_cwd);
    let background = arguments
        .get("background")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let log_file = arguments.get("logFile").and_then(Value::as_str);

    let ctx = BatchContext {
        global_cwd,
        timeout,
        global_shell,
        background,
        log_file,
        run_parallel,
        continue_on_error,
    };

    let commands_array = if let Some(cmd_str) = arguments.get("command").and_then(Value::as_str) {
        vec![json!({ "command": cmd_str })]
    } else if let Some(cmds) = arguments.get("commands").and_then(Value::as_array) {
        cmds.clone()
    } else {
        return Err("Either 'command' or 'commands' must be provided".to_string());
    };

    run_batch_commands(&commands_array, ctx).await
}

async fn run_batch_commands(cmds: &[Value], ctx: BatchContext<'_>) -> Result<Value, String> {
    if ctx.run_parallel && !ctx.continue_on_error && cmds.len() > 1 {
        return Err(
            "`continueOnError: false` has no effect with `runParallel: true`. \
             Either set `continueOnError: true` or disable `runParallel`."
                .to_string(),
        );
    }

    if ctx.run_parallel {
        run_parallel_commands(cmds, &ctx).await
    } else {
        run_sequential_commands(cmds, &ctx).await
    }
}

async fn run_parallel_commands(cmds: &[Value], ctx: &BatchContext<'_>) -> Result<Value, String> {
    let mut futures_list: Vec<Pin<Box<dyn Future<Output = Value>>>> = Vec::new();
    for item in cmds {
        let cmd = match item.get("command").and_then(Value::as_str) {
            Some(c) if !c.is_empty() => c.to_string(),
            _ => {
                futures_list.push(Box::pin(async move {
                    json!({ "error": "Missing or empty 'command' field in batch item" })
                }) as _);
                continue;
            }
        };
        let cwd = item
            .get("cwd")
            .and_then(Value::as_str)
            .or(ctx.global_cwd)
            .map(String::from);
        let shell = item
            .get("shell")
            .and_then(Value::as_str)
            .or(ctx.global_shell)
            .map(String::from);
        let bg = item
            .get("background")
            .and_then(Value::as_bool)
            .unwrap_or(ctx.background);
        let lf = item
            .get("logFile")
            .and_then(Value::as_str)
            .or(ctx.log_file)
            .map(String::from);
        let timeout = ctx.timeout;

        futures_list.push(Box::pin(async move {
            let res = execute_with_shell_choice(
                &cmd,
                cwd.as_deref(),
                timeout,
                shell.as_deref(),
                bg,
                lf.as_deref(),
            )
            .await;
            json!({
                "command": cmd,
                "result": match res { Ok(r) => json!(r), Err(e) => json!({"error": e}) }
            })
        }) as _);
    }
    let results = future::join_all(futures_list).await;
    Ok(json!([{ "type": "text", "text": serde_json::to_string(&results).unwrap_or_default() }]))
}

async fn run_sequential_commands(cmds: &[Value], ctx: &BatchContext<'_>) -> Result<Value, String> {
    let mut results = Vec::new();
    for item in cmds {
        let cmd = match item.get("command").and_then(Value::as_str) {
            Some(c) if !c.is_empty() => c,
            _ => {
                results.push(json!({ "error": "Missing or empty 'command' field in batch item" }));
                if !ctx.continue_on_error {
                    break;
                }
                continue;
            }
        };
        let cwd = item.get("cwd").and_then(Value::as_str).or(ctx.global_cwd);
        let shell = item
            .get("shell")
            .and_then(Value::as_str)
            .or(ctx.global_shell);
        let background = item
            .get("background")
            .and_then(Value::as_bool)
            .unwrap_or(ctx.background);
        let log_file = item.get("logFile").and_then(Value::as_str).or(ctx.log_file);

        let res =
            match execute_with_shell_choice(cmd, cwd, ctx.timeout, shell, background, log_file)
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    results.push(json!({ "command": cmd, "error": e }));
                    if !ctx.continue_on_error {
                        results.push(json!({ "status": "Stopped due to command failure" }));
                        break;
                    }
                    continue;
                }
            };
        results.push(json!({ "command": cmd, "result": json!(res) }));
        if res.exit_code != 0 && !ctx.continue_on_error {
            results.push(json!({ "status": "Stopped due to command failure" }));
            break;
        }
    }
    Ok(json!([{ "type": "text", "text": serde_json::to_string(&results).unwrap_or_default() }]))
}
