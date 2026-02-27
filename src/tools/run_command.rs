use crate::process::{CommandResponse, exec_cmd, exec_powershell};
use futures::future;
use serde_json::{Value, json};

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
                            "shell": { "type": "string", "description": "Optional specific shell for this command." }
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
                "runParallel": { "type": "boolean", "description": "Execute multiple commands in parallel." },
                "background": { "type": "boolean", "description": "Run the command in the background." },
                "waitForOutput": { "type": "number", "description": "Time to wait for child process output (ms) to detect early exit." },
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
    wait_ms: u64,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    let shell = shell_opt.unwrap_or("");
    if shell == "powershell" || shell == "pwsh" || (cfg!(windows) && shell == "ps") {
        exec_powershell(
            command_str,
            cwd_opt,
            timeout_ms,
            background,
            wait_ms,
            log_file,
        )
        .await
    } else {
        exec_cmd(
            command_str,
            cwd_opt,
            timeout_ms,
            shell_opt,
            background,
            wait_ms,
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
    wait_ms: u64,
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
    let wait_ms = arguments
        .get("waitForOutput")
        .and_then(Value::as_u64)
        .unwrap_or(1000);
    let log_file = arguments.get("logFile").and_then(Value::as_str);

    // Case 1: Single command
    if let Some(cmd_str) = arguments.get("command").and_then(Value::as_str) {
        return run_single_command(
            cmd_str,
            global_cwd,
            timeout,
            global_shell,
            background,
            wait_ms,
            log_file,
        )
        .await;
    }

    // Case 2: Batch commands
    if let Some(cmds) = arguments.get("commands").and_then(Value::as_array) {
        let ctx = BatchContext {
            global_cwd,
            timeout,
            global_shell,
            background,
            wait_ms,
            log_file,
            run_parallel,
            continue_on_error,
        };
        return run_batch_commands(cmds, ctx).await;
    }

    Err("Either 'command' or 'commands' must be provided".to_string())
}

async fn run_single_command(
    cmd_str: &str,
    global_cwd: Option<&str>,
    timeout: u64,
    global_shell: Option<&str>,
    background: bool,
    wait_ms: u64,
    log_file: Option<&str>,
) -> Result<Value, String> {
    let res = execute_with_shell_choice(
        cmd_str,
        global_cwd,
        timeout,
        global_shell,
        background,
        wait_ms,
        log_file,
    )
    .await?;
    Ok(json!([{ "type": "text", "text": serde_json::to_string(&res).unwrap_or_default() }]))
}

async fn run_batch_commands(cmds: &[Value], ctx: BatchContext<'_>) -> Result<Value, String> {
    if ctx.run_parallel {
        let mut futures_list = Vec::new();
        for item in cmds {
            let cmd = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
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
            let bg = ctx.background;
            let w = ctx.wait_ms;
            let lf = ctx.log_file.map(String::from);
            let timeout = ctx.timeout;

            futures_list.push(async move {
                let res = execute_with_shell_choice(
                    &cmd,
                    cwd.as_deref(),
                    timeout,
                    shell.as_deref(),
                    bg,
                    w,
                    lf.as_deref(),
                )
                .await;
                json!({
                    "command": cmd,
                    "result": match res { Ok(r) => json!(r), Err(e) => json!({"error": e}) }
                })
            });
        }
        let results = future::join_all(futures_list).await;
        return Ok(
            json!([{ "type": "text", "text": serde_json::to_string(&results).unwrap_or_default() }]),
        );
    }

    let mut results = Vec::new();
    for item in cmds {
        let cmd = item.get("command").and_then(Value::as_str).unwrap_or("");
        let cwd = item.get("cwd").and_then(Value::as_str).or(ctx.global_cwd);
        let shell = item
            .get("shell")
            .and_then(Value::as_str)
            .or(ctx.global_shell);

        let res = execute_with_shell_choice(
            cmd,
            cwd,
            ctx.timeout,
            shell,
            ctx.background,
            ctx.wait_ms,
            ctx.log_file,
        )
        .await?;
        let success = res.exit_code == 0;

        results.push(json!({ "command": cmd, "result": json!(res) }));
        if !success && !ctx.continue_on_error {
            results.push(json!({"status": "Stopped due to command failure"}));
            break;
        }
    }
    Ok(json!([{ "type": "text", "text": serde_json::to_string(&results).unwrap_or_default() }]))
}
