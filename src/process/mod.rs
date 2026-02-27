use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
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
const CREATE_NO_WINDOW: u32 = 0x08000000;

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

    for blocked in blacklist.iter() {
        if lower_cmd.contains(blocked) {
            return false;
        }
    }
    true
}

/// Thực hiện Force Kill theo Tree tiến trình
#[cfg(windows)]
pub fn force_kill_tree(pid: u32) {
    let mut kill_cmd = std::process::Command::new("taskkill");
    kill_cmd
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _ = kill_cmd.status();
}

#[cfg(not(windows))]
pub fn force_kill_tree(pid: u32) {
    // Trên Unix, pkill -P <pid> giết các con,
    // nhưng tốt nhất là kill PID chính trước rồi mới kill lũ con hoặc dùng group.
    // Đơn giản nhất là kill PID chính.
    let mut kill_cmd = std::process::Command::new("kill");
    kill_cmd
        .args(["-9", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = kill_cmd.status();
}

/// Helper function to limit string length
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

/// Thực thi một lệnh với shell tùy chọn và anti-hang protections
pub async fn exec_cmd(
    command_str: &str,
    cwd_opt: Option<&str>,
    timeout_ms: u64,
    shell_opt: Option<&str>,
    background: bool,
    wait_ms: u64,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    if !is_safe_command(command_str) {
        return Ok(CommandResponse {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 1,
            timeout: false,
            error: Some("Command rejected by security blacklist".to_string()),
        });
    }

    if let Some(cwd) = cwd_opt
        && !Path::new(cwd).is_dir() {
            return Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Invalid working directory: {}", cwd)),
            });
        }

    let max_output_bytes = 10 * 1024 * 1024; // 10MB limit

    let mut _cmd = if cfg!(windows) {
        let shell = shell_opt.unwrap_or("cmd.exe");
        let mut c = tokio::process::Command::new(shell);
        if shell.ends_with("cmd.exe") || shell == "cmd" {
            c.args(["/c", command_str]);
        } else {
            c.args(["-c", command_str]);
        }
        #[cfg(windows)]
        c.creation_flags(CREATE_NO_WINDOW);
        c
    } else {
        let shell = shell_opt.unwrap_or("sh");
        let mut c = tokio::process::Command::new(shell);
        c.args(["-c", command_str]);
        c
    };

    if let Some(log_path) = log_file {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;
        let err_file = file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file: {}", e))?;
        _cmd.stdout(Stdio::from(file));
        _cmd.stderr(Stdio::from(err_file));
    } else {
        _cmd.stdout(Stdio::piped());
        _cmd.stderr(Stdio::piped());
    }

    _cmd.stdin(Stdio::piped());

    if let Some(cwd) = cwd_opt {
        _cmd.current_dir(cwd);
    }

    let mut child = match _cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Spawn failed: {}", e)),
            });
        }
    };

    let pid = child.id().unwrap_or(0);
    drop(child.stdin.take());

    // Nếu chạy ngầm, chúng ta có thể muốn đợi một lát để xem nó có chết ngay không.
    // Nếu không, trả về PID.
    if background {
        // Đợi trong thời gian ngắn
        match timeout(Duration::from_millis(wait_ms), child.wait()).await {
            Ok(Ok(status)) => {
                // Nó chết sớm
                return Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: status.code().unwrap_or(1),
                    timeout: false,
                    error: Some("Process exited prematurely".to_string()),
                });
            }
            Ok(Err(e)) => {
                return Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: false,
                    error: Some(format!("Wait failed: {}", e)),
                });
            }
            Err(_) => {
                // Nó vẫn đang chạy! Thành công cho chế độ background
                let msg = if let Some(path) = log_file {
                    format!(
                        "Process started in background. PID: {}. Logs: {}",
                        pid, path
                    )
                } else {
                    format!("Process started in background. PID: {}", pid)
                };
                return Ok(CommandResponse {
                    stdout: msg,
                    stderr: String::new(),
                    exit_code: 0,
                    timeout: false,
                    error: None,
                });
            }
        }
    }

    // Đợi process chạy cho đến khi kết thúc (Behavior cũ)
    if log_file.is_some() {
        let status = timeout(Duration::from_millis(timeout_ms), child.wait()).await;
        match status {
            Ok(Ok(s)) => Ok(CommandResponse {
                stdout: "Output redirected to file".to_string(),
                stderr: String::new(),
                exit_code: s.code().unwrap_or(1),
                timeout: false,
                error: None,
            }),
            Ok(Err(e)) => Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Wait failed: {}", e)),
            }),
            Err(_) => {
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {}ms timeout", timeout_ms)),
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
                    stdout: truncate_to_bytes(&stdout_str, max_output_bytes),
                    stderr: truncate_to_bytes(&stderr_str, max_output_bytes),
                    exit_code: output.status.code().unwrap_or(1),
                    timeout: false,
                    error: None,
                })
            }
            Ok(Err(e)) => Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Wait failed: {}", e)),
            }),
            Err(_) => {
                // Hết hạn thời gian (Timeout), Kill tree!
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {}ms timeout", timeout_ms)),
                })
            }
        }
    }
}

/// Chạy PowerShell an toàn với Base64 Encoded Command
pub async fn exec_powershell(
    command_str: &str,
    cwd_opt: Option<&str>,
    timeout_ms: u64,
    background: bool,
    wait_ms: u64,
    log_file: Option<&str>,
) -> Result<CommandResponse, String> {
    if !is_safe_command(command_str) {
        return Ok(CommandResponse {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 1,
            timeout: false,
            error: Some("Command rejected by security blacklist".to_string()),
        });
    }

    if let Some(cwd) = cwd_opt
        && !Path::new(cwd).is_dir() {
            return Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Invalid working directory: {}", cwd)),
            });
        }

    let max_output_bytes = 5 * 1024 * 1024;

    #[cfg(windows)]
    let mut _cmd = {
        let utf16_bytes: Vec<u16> = command_str.encode_utf16().collect();
        let mut u8_bytes = Vec::with_capacity(utf16_bytes.len() * 2);
        for b in utf16_bytes {
            u8_bytes.push((b & 0xFF) as u8);
            u8_bytes.push((b >> 8) as u8);
        }
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let encoded = STANDARD.encode(&u8_bytes);

        let mut c = tokio::process::Command::new("powershell.exe");
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
    };

    #[cfg(not(windows))]
    let mut _cmd = {
        let utf16_bytes: Vec<u16> = command_str.encode_utf16().collect();
        let mut u8_bytes = Vec::with_capacity(utf16_bytes.len() * 2);
        for b in utf16_bytes {
            u8_bytes.push((b & 0xFF) as u8);
            u8_bytes.push((b >> 8) as u8);
        }
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let encoded = STANDARD.encode(&u8_bytes);

        let mut c = tokio::process::Command::new("pwsh");
        c.args(["-NonInteractive", "-NoProfile", "-EncodedCommand", &encoded]);
        c
    };

    if let Some(log_path) = log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;
        let err_file = file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file: {}", e))?;
        _cmd.stdout(Stdio::from(file));
        _cmd.stderr(Stdio::from(err_file));
    } else {
        _cmd.stdout(Stdio::piped());
        _cmd.stderr(Stdio::piped());
    }

    _cmd.stdin(Stdio::piped());

    if let Some(cwd) = cwd_opt {
        _cmd.current_dir(cwd);
    }

    let mut child = match _cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Spawn failed: {}", e)),
            });
        }
    };

    let pid = child.id().unwrap_or(0);
    drop(child.stdin.take());

    if background {
        match timeout(Duration::from_millis(wait_ms), child.wait()).await {
            Ok(Ok(status)) => {
                return Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: status.code().unwrap_or(1),
                    timeout: false,
                    error: Some("Process exited prematurely".to_string()),
                });
            }
            Ok(Err(e)) => {
                return Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: false,
                    error: Some(format!("Wait failed: {}", e)),
                });
            }
            Err(_) => {
                let msg = if let Some(path) = log_file {
                    format!(
                        "PowerShell process started in background. PID: {}. Logs: {}",
                        pid, path
                    )
                } else {
                    format!("PowerShell process started in background. PID: {}", pid)
                };
                return Ok(CommandResponse {
                    stdout: msg,
                    stderr: String::new(),
                    exit_code: 0,
                    timeout: false,
                    error: None,
                });
            }
        }
    }

    if log_file.is_some() {
        let status = timeout(Duration::from_millis(timeout_ms), child.wait()).await;
        match status {
            Ok(Ok(s)) => Ok(CommandResponse {
                stdout: "Output redirected to file".to_string(),
                stderr: String::new(),
                exit_code: s.code().unwrap_or(1),
                timeout: false,
                error: None,
            }),
            Ok(Err(e)) => Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Wait failed: {}", e)),
            }),
            Err(_) => {
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {}ms timeout", timeout_ms)),
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
                    stdout: truncate_to_bytes(&stdout_str, max_output_bytes),
                    stderr: truncate_to_bytes(&stderr_str, max_output_bytes),
                    exit_code: output.status.code().unwrap_or(1),
                    timeout: false,
                    error: None,
                })
            }
            Ok(Err(e)) => Ok(CommandResponse {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 1,
                timeout: false,
                error: Some(format!("Wait failed: {}", e)),
            }),
            Err(_) => {
                if pid > 0 {
                    force_kill_tree(pid);
                }
                Ok(CommandResponse {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: 1,
                    timeout: true,
                    error: Some(format!("Killed after {}ms timeout", timeout_ms)),
                })
            }
        }
    }
}
