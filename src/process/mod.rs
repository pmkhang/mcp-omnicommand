use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::process::{Child, Command as TokioCommand};
use tokio::time::timeout;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timeout: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024; // 10MB

/// Best-effort safety check. NOT a security boundary —
/// simple string matching can be bypassed with minor variations
/// (e.g. extra spaces, different flags). Callers should not rely
/// on this as the sole protection against destructive commands.
pub fn is_safe_command(cmd: &str) -> bool {
    let lower_cmd = cmd.to_lowercase();
    let blacklist = [
        "rm -rf",
        "rmdir /s",
        "del /f",
        "format ",
        "diskpart",
        "reg add",
        "reg delete",
        "net user",
        "net localgroup",
        "stop-computer",
        "restart-computer",
        "remove-item -recurse -force",
    ];

    for blocked in &blacklist {
        if lower_cmd.contains(blocked) {
            return false;
        }
    }
    true
}

#[cfg(windows)]
pub fn force_kill_tree(pid: u32) {
    let mut kill_cmd = Command::new("taskkill");
    kill_cmd
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _ = kill_cmd.status();
}

#[cfg(not(windows))]
pub fn force_kill_tree(pid: u32) {
    // Try to kill entire process group first (negative PID = process group)
    let _ = Command::new("kill")
        .args(["-9", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // Fallback: kill the process itself in case it has no process group
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn validate_args(command_str: &str, cwd_opt: Option<&str>) -> Result<(), String> {
    if !is_safe_command(command_str) {
        return Err("Command rejected by security blacklist".to_string());
    }
    if let Some(cwd) = cwd_opt.filter(|c| !Path::new(c).is_dir()) {
        return Err(format!("Invalid working directory: {cwd}"));
    }
    Ok(())
}

fn setup_stdio(cmd: &mut TokioCommand, log_file: Option<&str>) -> Result<Option<String>, String> {
    if let Some(log_path) = log_file {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| format!("Failed to open log file: {e}"))?;
        let err_file = file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file: {e}"))?;
        cmd.stdout(Stdio::from(file));
        cmd.stderr(Stdio::from(err_file));
        Ok(Some(log_path.to_string()))
    } else {
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        Ok(None)
    }
}

async fn monitor_process(
    mut child: Child,
    timeout_ms: u64,
    background: bool,
    log_path_opt: Option<String>,
) -> Result<CommandResponse, String> {
    let pid = child.id().unwrap_or(0);
    drop(child.stdin.take());

    if background {
        let msg = if let Some(path) = log_path_opt {
            format!("Process started in background. PID: {pid}. Logs: {path}")
        } else {
            format!("Process started in background. PID: {pid}")
        };
        return Ok(CommandResponse {
            stdout: msg,
            stderr: String::new(),
            exit_code: 0,
            timeout: false,
            error: None,
        });
    }

    if log_path_opt.is_some() {
        let status = timeout(Duration::from_millis(timeout_ms), child.wait()).await;
        match status {
            Ok(Ok(s)) => Ok(CommandResponse {
                stdout: "Output redirected to file".to_string(),
                stderr: String::new(),
                exit_code: s.code().unwrap_or(0),
                timeout: false,
                error: None,
            }),
            Ok(Err(e)) => Err(format!("Wait failed: {e}")),
            Err(_) => {
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {timeout_ms}ms timeout")),
                })
            }
        }
    } else {
        let result = timeout(Duration::from_millis(timeout_ms), child.wait_with_output()).await;
        match result {
            Ok(Ok(output)) => {
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                let stderr_str = String::from_utf8_lossy(&output.stderr);

                Ok(CommandResponse {
                    stdout: truncate_to_bytes(&stdout_str, MAX_OUTPUT_BYTES),
                    stderr: truncate_to_bytes(&stderr_str, MAX_OUTPUT_BYTES),
                    exit_code: output.status.code().unwrap_or(0),
                    timeout: false,
                    error: None,
                })
            }
            Ok(Err(e)) => Err(format!("Wait failed: {e}")),
            Err(_) => {
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {timeout_ms}ms timeout")),
                })
            }
        }
    }
}

pub async fn exec_cmd(
    command_str: &str,
    cwd_opt: Option<&str>,
    timeout_ms: u64,
    shell_opt: Option<&str>,
    background: bool,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    validate_args(command_str, cwd_opt)?;

    let mut cmd = if cfg!(windows) {
        let shell = shell_opt.unwrap_or("cmd.exe");
        let mut c = TokioCommand::new(shell);
        if shell.ends_with("cmd.exe") || shell == "cmd" {
            c.args(["/c", command_str]);
        } else {
            c.args(["-c", command_str]);
        }
        c.creation_flags(CREATE_NO_WINDOW);
        c
    } else {
        let shell = shell_opt.unwrap_or("sh");
        let mut c = TokioCommand::new(shell);
        c.args(["-c", command_str]);
        c
    };

    let log_path_opt = setup_stdio(&mut cmd, log_file)?;
    cmd.stdin(Stdio::piped());
    if let Some(cwd) = cwd_opt {
        cmd.current_dir(cwd);
    }

    let child = cmd.spawn().map_err(|e| format!("Spawn failed: {e}"))?;
    monitor_process(child, timeout_ms, background, log_path_opt).await
}

pub async fn exec_powershell(
    command_str: &str,
    cwd_opt: Option<&str>,
    timeout_ms: u64,
    background: bool,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    validate_args(command_str, cwd_opt)?;

    let u8_bytes: Vec<u8> = command_str
        .encode_utf16()
        .flat_map(|b| [(b & 0xFF) as u8, (b >> 8) as u8])
        .collect();
    let encoded = STANDARD.encode(&u8_bytes);

    let mut cmd = if cfg!(windows) {
        let mut c = TokioCommand::new("powershell.exe");
        c.args([
            "-NonInteractive",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
            &encoded,
        ]);
        c.creation_flags(CREATE_NO_WINDOW);
        c
    } else {
        let mut c = TokioCommand::new("pwsh");
        c.args(["-NonInteractive", "-NoProfile", "-EncodedCommand", &encoded]);
        c
    };

    let log_path_opt = setup_stdio(&mut cmd, log_file)?;
    cmd.stdin(Stdio::piped());
    if let Some(cwd) = cwd_opt {
        cmd.current_dir(cwd);
    }

    let child = cmd.spawn().map_err(|e| format!("Spawn failed: {e}"))?;
    monitor_process(child, timeout_ms, background, log_path_opt).await
}
