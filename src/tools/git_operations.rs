use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::process::Command;
use tokio::task::spawn_blocking;

#[derive(Serialize)]
struct FileStatus {
    pub path: String,
    pub status: String, // "added", "modified", "deleted", "renamed"
}

#[derive(Serialize)]
struct GitStatus {
    pub branch: String,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: Vec<FileStatus>,
    pub modified: Vec<FileStatus>,
    pub untracked: Vec<String>,
    pub conflicts: Vec<String>,
}

#[derive(Serialize)]
struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

#[derive(Serialize)]
struct GitDiffStat {
    pub file: String,
    pub insertions: u32,
    pub deletions: u32,
    pub change_type: String, // "modified", "added", "deleted", "renamed"
    pub binary: bool,
}

#[derive(Serialize)]
struct GitBranch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub last_commit: Option<String>,
}

#[derive(Serialize)]
struct GitAddResult {
    pub staged_count: u32,
    pub newly_staged_count: u32,
    pub files: Vec<String>,
}

#[derive(Serialize)]
struct GitCommitResult {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub insertions: u32,
    pub deletions: u32,
    pub files_changed: u32,
}

#[derive(Serialize)]
struct GitPushResult {
    pub success: bool,
    pub remote: String,
    pub branch: String,
    pub summary: String,
    pub new_branch: bool,
}

#[derive(Serialize)]
struct GitPullResult {
    pub success: bool,
    pub summary: String,
    pub insertions: u32,
    pub deletions: u32,
    pub files_changed: u32,
    pub conflicts: Vec<String>,
    pub up_to_date: bool,
    pub fast_forward: bool,
}

#[derive(Serialize)]
struct GitCheckoutResult {
    pub ref_name: String,
    pub previous_ref: String,
    pub new_branch: bool,
    pub detached: bool,
}

struct GitRunner {
    cwd: String,
}

impl GitRunner {
    fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_string(),
        }
    }

    fn run(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .map_err(|e| {
                if e.kind() == ErrorKind::NotFound {
                    "git not found in PATH".to_string()
                } else {
                    format!("Failed to run git: {e}")
                }
            })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn run_with_stderr(&self, args: &[&str]) -> (String, String, bool) {
        match Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                (stdout, stderr, output.status.success())
            }
            Err(_) => (String::new(), "git not found in PATH".to_string(), false),
        }
    }

    fn ensure_git_repo(&self) -> Result<(), String> {
        self.run(&["rev-parse", "--git-dir"])
            .map(|_| ())
            .map_err(|_| format!("Not a git repository: {}", self.cwd))
    }
}

pub fn info() -> Value {
    json!({
        "name": "git_operations",
        "description": "Perform comprehensive Git operations.\n- status: Get working tree status\n- log: View commit history\n- diff: Show changes\n- branch: List, create, rename, or delete branches\n- add: Stage file contents\n- commit: Record changes\n- push: Update remote refs\n- pull: Fetch from and integrate with another repository\n- checkout: Switch branches or restore files\n- stash: Stash changes (list, save, pop, drop, clear)\n- reset: Reset HEAD to specified state (soft, mixed, hard) or reset specific files\n- merge: Join two histories together (merge, abort) reporting conflicts\n- fetch: Download objects and refs from remote\n- tag: Create (annotated/lightweight), list, or delete tags",
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
        },
        "inputSchema": {
            "type": "object",
            "properties": {
                "op": {
                    "type": "string",
                    "enum": ["status", "log", "diff", "branch", "add", "commit", "push", "pull", "checkout", "stash", "reset", "merge", "fetch", "tag"],
                    "description": "Git operation to perform"
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory (git repo path). Defaults to server cwd."
                },
                "limit": {
                    "type": "number",
                    "description": "[log] Max commits to return. Default: 20"
                },
                "target": {
                    "type": "string",
                    "description": "[diff] Ref, file path, or commit range (e.g. HEAD~1..HEAD)"
                },
                "staged": {
                    "type": "boolean",
                    "description": "[diff] Show staged diff. Default: false"
                },
                "full": {
                    "type": "boolean",
                    "description": "[diff] Include full patch content. Default: false"
                },
                "action": {
                    "type": "string",
                    "enum": ["list", "create", "rename", "delete", "save", "pop", "drop", "clear", "abort"],
                    "description": "Action to perform for branch, stash, tag, or merge ops."
                },
                "name": {
                    "type": "string",
                    "description": "[branch] Branch name for create/rename/delete"
                },
                "new_name": {
                    "type": "string",
                    "description": "[branch] New name when renaming"
                },
                "force": {
                    "type": "boolean",
                    "description": "[branch delete / push] Force operation. Default: false"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "[add] Files to stage. Use [\".\"] to stage all."
                },
                "message": {
                    "type": "string",
                    "description": "[commit] Commit message"
                },
                "remote": {
                    "type": "string",
                    "description": "[push/pull] Remote name. Default: origin"
                },
                "ref": {
                    "type": "string",
                    "description": "[checkout] Branch, tag, or commit to checkout"
                },
                "new_branch": {
                    "type": "boolean",
                    "description": "[checkout] Create new branch (-b). Default: false"
                },
                "restore": {
                    "type": "string",
                    "description": "[checkout] File path to restore (git checkout -- <file>)"
                },
                "stash_index": {
                    "type": "number",
                    "description": "[stash] Index for pop/drop. Default: 0"
                },
                "mode": {
                    "type": "string",
                    "enum": ["soft", "mixed", "hard"],
                    "description": "[reset] Reset mode. Default: mixed"
                }
            },
            "required": ["op"]
        }
    })
}

pub async fn run(arguments: &Value, default_cwd: Option<&str>) -> Result<Value, String> {
    let arguments = arguments.clone();
    let default_cwd = default_cwd.map(String::from);
    spawn_blocking(move || run_sync(&arguments, default_cwd.as_deref()))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

fn run_sync(arguments: &Value, default_cwd: Option<&str>) -> Result<Value, String> {
    let op = arguments
        .get("op")
        .and_then(Value::as_str)
        .ok_or("op is required")?;
    let cwd = arguments
        .get("cwd")
        .and_then(Value::as_str)
        .or(default_cwd)
        .ok_or("cwd is required (no default available)")?;

    let runner = GitRunner::new(cwd);
    runner.ensure_git_repo()?;

    let result: Value = match op {
        "status" => run_status(&runner)?,
        "log" => run_log(&runner, arguments)?,
        "diff" => run_diff(&runner, arguments)?,
        "branch" => run_branch(&runner, arguments)?,
        "add" => run_add(&runner, arguments)?,
        "commit" => run_commit(&runner, arguments)?,
        "push" => run_push(&runner, arguments)?,
        "pull" => run_pull(&runner, arguments)?,
        "checkout" => run_checkout(&runner, arguments)?,
        "stash" => run_stash(&runner, arguments)?,
        "reset" => run_reset(&runner, arguments)?,
        "merge" => run_merge(&runner, arguments)?,
        "fetch" => run_fetch(&runner, arguments)?,
        "tag" => run_tag(&runner, arguments)?,
        _ => return Err(format!("Unknown op: {op}")),
    };

    Ok(json!([{
        "type": "text",
        "text": serde_json::to_string_pretty(&result).unwrap_or_default()
    }]))
}

fn map_status_char(c: char) -> &'static str {
    match c {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        _ => "unknown",
    }
}

fn run_status(runner: &GitRunner) -> Result<Value, String> {
    let out = runner.run(&["status", "--porcelain=v2", "--branch"])?;
    let mut status = GitStatus {
        branch: String::new(),
        upstream: None,
        ahead: 0,
        behind: 0,
        staged: Vec::new(),
        modified: Vec::new(),
        untracked: Vec::new(),
        conflicts: Vec::new(),
    };

    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            status.branch = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            status.upstream = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 2 {
                if let Some(a) = parts[0].strip_prefix("+") {
                    status.ahead = a.parse().unwrap_or(0);
                }
                if let Some(b) = parts[1].strip_prefix("-") {
                    status.behind = b.parse().unwrap_or(0);
                }
            }
        } else if line.starts_with('1') || line.starts_with('2') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                let xy = parts[1]; // e.g. "M." or ".M" or "A "
                let mut chars = xy.chars();
                let x = chars.next().unwrap_or('.');
                let y = chars.next().unwrap_or('.');

                if let Some(last_part) = parts.last() {
                    let path = last_part.to_string();

                    if x != '.' {
                        status.staged.push(FileStatus {
                            path: path.clone(),
                            status: map_status_char(x).to_string(),
                        });
                    }
                    if y != '.' {
                        status.modified.push(FileStatus {
                            path,
                            status: map_status_char(y).to_string(),
                        });
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("u ") {
            // Unmerged (conflict)
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let Some(path) = parts.last() {
                status.conflicts.push(path.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("? ") {
            status.untracked.push(rest.to_string());
        }
    }

    Ok(json!(status))
}

fn run_log(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let limit_usize = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(20);
    let limit_str = format!("-{limit_usize}");
    let out = runner.run(&[
        "log",
        "--pretty=format:%H%x1f%h%x1f%an%x1f%ai%x1f%s",
        &limit_str,
    ])?;

    let mut commits = Vec::new();
    if out.is_empty() {
        return Ok(json!(commits));
    }

    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(5, '\x1f').collect();
        if parts.len() == 5 {
            commits.push(GitCommit {
                hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                author: parts[2].to_string(),
                date: parts[3].to_string(),
                message: parts[4].to_string(),
            });
        }
    }
    Ok(json!(commits))
}

fn run_diff(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let mut args = vec!["diff"];
    let staged = arguments
        .get("staged")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if staged {
        args.push("--cached");
    }

    let target = arguments.get("target").and_then(|v| v.as_str());
    let mut numstat_args = args.clone();
    numstat_args.push("--numstat");
    if let Some(t) = target {
        numstat_args.push(t);
        args.push(t);
    }

    let stat_out = runner.run(&numstat_args)?;

    let mut namestatus_args: Vec<&str> = if staged {
        vec!["diff", "--cached"]
    } else {
        vec!["diff"]
    };
    namestatus_args.push("--name-status");
    if let Some(t) = target {
        namestatus_args.push(t);
    }
    let namestatus_out = runner.run(&namestatus_args)?;

    // Build map: file -> change_type
    let mut change_type_map: HashMap<String, String> = HashMap::new();
    for line in namestatus_out.lines() {
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() >= 2 {
            let status_char = parts[0].chars().next().unwrap_or('M');
            let file = parts[parts.len() - 1].trim().to_string();
            let change_type = match status_char {
                'A' => "added",
                'D' => "deleted",
                'R' => "renamed",
                _ => "modified",
            };
            change_type_map.insert(file, change_type.to_string());
        }
    }

    let full = arguments
        .get("full")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut stats = Vec::new();
    for line in stat_out.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        if parts.len() == 3 {
            let binary = parts[0].trim() == "-" || parts[1].trim() == "-";
            let insertions = parts[0].parse::<u32>().unwrap_or(0);
            let deletions = parts[1].parse::<u32>().unwrap_or(0);
            let file = parts[2].trim().to_string();

            stats.push(GitDiffStat {
                file: file.clone(),
                insertions,
                deletions,
                change_type: change_type_map
                    .get(&file)
                    .cloned()
                    .unwrap_or_else(|| "modified".to_string()),
                binary,
            });
        }
    }

    let mut result = json!({
        "stats": stats,
    });

    if full {
        let full_out = runner.run(&args)?;
        result["patch"] = json!(full_out);
    }

    Ok(result)
}

fn run_branch(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list");
    let name = arguments.get("name").and_then(|v| v.as_str());

    match action {
        "list" => {
            let out = runner.run(&["branch", "-vv"]).unwrap_or_default();
            let mut branches = Vec::new();
            for line in out.lines() {
                if line.is_empty() {
                    continue;
                }
                let current = line.starts_with('*');
                let stripped = line.trim_start_matches(|c: char| c == '*' || c.is_whitespace());

                let parts: Vec<&str> = stripped.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let bname = parts[0].to_string();
                let short_hash = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };

                let mut upstream = None;
                let mut ahead = None;
                let mut behind = None;

                if let Some(idx_open) = stripped.find('[')
                    && let Some(idx_close) = stripped[idx_open..].find(']')
                {
                    let inside = &stripped[idx_open + 1..idx_open + idx_close];
                    let bracket_parts: Vec<&str> = inside.split(':').collect();
                    upstream = Some(bracket_parts[0].trim().to_string());

                    if bracket_parts.len() > 1 {
                        let ab = bracket_parts[1].trim();
                        for part in ab.split(',') {
                            let part = part.trim();
                            if let Some(a) = part.strip_prefix("ahead ") {
                                ahead = a.parse().ok();
                            }
                            if let Some(b) = part.strip_prefix("behind ") {
                                behind = b.parse().ok();
                            }
                        }
                    }
                }

                branches.push(GitBranch {
                    name: bname,
                    current,
                    upstream,
                    ahead,
                    behind,
                    last_commit: short_hash,
                });
            }
            Ok(json!(branches))
        }
        "create" => {
            let branch_name = name.ok_or("name is required for branch create")?;
            runner.run(&["branch", branch_name])?;
            Ok(json!({"success": true, "branch": branch_name}))
        }
        "rename" => {
            let old_name = name.ok_or("name is required for branch rename")?;
            let new_name = arguments
                .get("new_name")
                .and_then(|v| v.as_str())
                .ok_or("new_name is required for branch rename")?;
            runner.run(&["branch", "-m", old_name, new_name])?;
            Ok(json!({"success": true, "old_name": old_name, "new_name": new_name}))
        }
        "delete" => {
            let branch_name = name.ok_or("name is required for branch delete")?;
            let force = arguments
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let d_flag = if force { "-D" } else { "-d" };
            match runner.run(&["branch", d_flag, branch_name]) {
                Ok(out) => Ok(json!({"success": true, "branch": branch_name, "summary": out})),
                Err(e) => {
                    let mut hint = e;
                    if !force {
                        hint.push_str(" (use force: true to force delete)");
                    }
                    Err(hint)
                }
            }
        }
        _ => Err(format!("Unknown action: {action}")),
    }
}

fn get_staged_files(runner: &GitRunner) -> Vec<String> {
    let out = runner
        .run(&["diff", "--name-only", "--cached"])
        .unwrap_or_default();
    out.lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn run_add(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let files_val = arguments
        .get("files")
        .and_then(|v| v.as_array())
        .ok_or("files array is required and must not be empty")?;
    if files_val.is_empty() {
        return Err("files array is required and must not be empty".to_string());
    }

    let mut files = Vec::new();
    for v in files_val {
        if let Some(s) = v.as_str() {
            files.push(s);
        }
    }

    let before = get_staged_files(runner);

    let mut args = vec!["add"];
    args.extend(files.iter());
    runner.run(&args)?;

    let after = get_staged_files(runner);

    let staged_count = u32::try_from(after.len()).unwrap_or(0);
    let newly_staged_count =
        u32::try_from(after.iter().filter(|f| !before.contains(f)).count()).unwrap_or(0);

    Ok(json!(GitAddResult {
        staged_count,
        newly_staged_count,
        files: after,
    }))
}

fn run_commit(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let message = arguments
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or("message is required for commit")?;
    if message.trim().is_empty() {
        return Err("message is required for commit".to_string());
    }

    let out = runner.run(&["commit", "-m", message])?;
    let full_hash = runner.run(&["rev-parse", "HEAD"]).unwrap_or_default();

    let mut short_hash = String::new();
    let mut insertions = 0;
    let mut deletions = 0;
    let mut files_changed = 0;

    let lines: Vec<&str> = out.lines().collect();
    if !lines.is_empty() {
        // Line 1: [branch abc1234] message
        if let Some(idx) = lines[0].find(']') {
            let inside = &lines[0][1..idx]; // "branch abc1234"
            let parts: Vec<&str> = inside.split_whitespace().collect();
            if parts.len() > 1 {
                short_hash = parts[1].to_string();
            }
        }

        // stats
        for line in &lines[1..] {
            if line.contains(" changed") {
                let parts: Vec<&str> = line.split(',').collect();
                for part in parts {
                    let part = part.trim();
                    if part.ends_with(" files changed") || part.ends_with(" file changed") {
                        if let Some(n) = part.split_whitespace().next() {
                            files_changed = n.parse().unwrap_or(0);
                        }
                    } else if part.ends_with(" insertions(+)") || part.ends_with(" insertion(+)") {
                        if let Some(n) = part.split_whitespace().next() {
                            insertions = n.parse().unwrap_or(0);
                        }
                    } else if (part.ends_with(" deletions(-)") || part.ends_with(" deletion(-)"))
                        && let Some(n) = part.split_whitespace().next()
                    {
                        deletions = n.parse().unwrap_or(0);
                    }
                }
            }
        }
    }

    Ok(json!(GitCommitResult {
        hash: full_hash,
        short_hash,
        message: message.to_string(),
        insertions,
        deletions,
        files_changed,
    }))
}

fn run_push(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let remote = arguments
        .get("remote")
        .and_then(|v| v.as_str())
        .unwrap_or("origin");
    let force = arguments
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut args = vec!["push", remote, "HEAD"];
    if force {
        args.push("--force");
    }

    let (stdout, stderr, success) = runner.run_with_stderr(&args);
    let summary = if stdout.is_empty() {
        stderr.clone()
    } else {
        stdout
    };
    let new_branch = stderr.contains("* [new branch]");
    let branch = runner.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;

    Ok(json!(GitPushResult {
        success,
        remote: remote.to_string(),
        branch,
        summary,
        new_branch,
    }))
}

fn run_pull(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let remote = arguments
        .get("remote")
        .and_then(|v| v.as_str())
        .unwrap_or("origin");

    if remote.is_empty() {
        return Err("remote must not be empty".to_string());
    }

    let (stdout, stderr, success) = runner.run_with_stderr(&["pull", remote]);
    let summary = if success && !stdout.is_empty() {
        stdout.clone()
    } else {
        stderr
    };

    let up_to_date = stdout.contains("Already up to date.");
    let fast_forward = stdout.contains("Fast-forward");

    let mut conflicts = Vec::new();
    let mut insertions = 0;
    let mut deletions = 0;
    let mut files_changed = 0;

    for line in stdout.lines() {
        if line.contains("CONFLICT") {
            conflicts.push(line.trim().to_string());
        }
        if line.contains(" changed") {
            let parts: Vec<&str> = line.split(',').collect();
            for part in parts {
                let part = part.trim();
                if part.ends_with(" files changed") || part.ends_with(" file changed") {
                    if let Some(n) = part.split_whitespace().next() {
                        files_changed = n.parse().unwrap_or(0);
                    }
                } else if part.ends_with(" insertions(+)") || part.ends_with(" insertion(+)") {
                    if let Some(n) = part.split_whitespace().next() {
                        insertions = n.parse().unwrap_or(0);
                    }
                } else if (part.ends_with(" deletions(-)") || part.ends_with(" deletion(-)"))
                    && let Some(n) = part.split_whitespace().next()
                {
                    deletions = n.parse().unwrap_or(0);
                }
            }
        }
    }

    Ok(json!(GitPullResult {
        success,
        summary,
        insertions,
        deletions,
        files_changed,
        conflicts,
        up_to_date,
        fast_forward,
    }))
}

fn run_checkout(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let ref_arg = arguments.get("ref").and_then(|v| v.as_str());
    let restore = arguments.get("restore").and_then(|v| v.as_str());

    if ref_arg.is_none() && restore.is_none() {
        return Err("ref or restore is required for checkout".to_string());
    }

    let previous_ref = runner.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;

    if let Some(file) = restore {
        runner.run(&["restore", file])?;
        return Ok(json!(GitCheckoutResult {
            ref_name: previous_ref.clone(),
            previous_ref,
            new_branch: false,
            detached: false,
        }));
    }

    let mut args = vec!["checkout"];
    let new_branch = arguments
        .get("new_branch")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if new_branch {
        args.push("-b");
    }
    let ref_name = if let Some(r) = ref_arg {
        args.push(r);
        r
    } else {
        return Err("ref is required for checkout".to_string());
    };

    runner.run(&args)?;

    let detached = runner.run(&["symbolic-ref", "-q", "HEAD"]).is_err();

    Ok(json!(GitCheckoutResult {
        ref_name: ref_name.to_string(),
        previous_ref,
        new_branch,
        detached,
    }))
}

fn run_stash(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list");

    match action {
        "list" => {
            let out = runner.run(&["stash", "list"]).unwrap_or_default();
            let mut stashes = Vec::new();
            // format: "stash@{0}: On main: my message"
            for line in out.lines() {
                if let Some(brace_start) = line.find('{')
                    && let Some(brace_end) = line.find('}')
                {
                    let index: u64 = line[brace_start + 1..brace_end].parse().unwrap_or(0);
                    // skip "}: " (3 chars) to get rest
                    let rest = if brace_end + 3 <= line.len() {
                        line[brace_end + 3..].trim()
                    } else {
                        ""
                    };
                    // rest = "On main: my message" or "WIP on main: my message"
                    let (branch, message) = if let Some(colon_idx) = rest.find(':') {
                        let b = rest[..colon_idx].trim().to_string();
                        let m = rest[colon_idx + 1..].trim().to_string();
                        (b, m)
                    } else {
                        (String::new(), rest.to_string())
                    };

                    stashes.push(json!({
                        "index": index,
                        "branch": branch,
                        "message": message,
                        "summary": line.to_string(),
                    }));
                }
            }
            Ok(json!(stashes))
        }
        "save" => {
            let msg = arguments.get("message").and_then(|v| v.as_str());
            let mut args = vec!["stash", "push"];
            if let Some(m) = msg {
                args.push("-m");
                args.push(m);
            }
            let (stdout, stderr, success) = runner.run_with_stderr(&args);
            if success {
                let summary = if stdout.is_empty() { stderr } else { stdout };
                Ok(json!({"success": true, "summary": summary}))
            } else {
                Err(stderr)
            }
        }
        "pop" => {
            let idx = arguments
                .get("stash_index")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let stash_ref = format!("stash@{{{idx}}}");
            let (stdout, stderr, success) = runner.run_with_stderr(&["stash", "pop", &stash_ref]);
            if success {
                let summary = if stdout.is_empty() { stderr } else { stdout };
                Ok(json!({"success": true, "summary": summary}))
            } else {
                Err(stderr)
            }
        }
        "drop" => {
            let idx = arguments
                .get("stash_index")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let stash_ref = format!("stash@{{{idx}}}");
            let out = runner.run(&["stash", "drop", &stash_ref])?;
            Ok(json!({"success": true, "summary": out}))
        }
        "clear" => {
            let out = runner.run(&["stash", "clear"])?;
            Ok(json!({"success": true, "summary": out}))
        }
        _ => Err(format!("Unknown stash action: {action}")),
    }
}

fn run_reset(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let target = arguments
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("HEAD");

    let files = arguments.get("files").and_then(|v| v.as_array());

    if let Some(files_array) = files
        && !files_array.is_empty()
    {
        let mut args = vec!["reset", target, "--"];
        for f in files_array {
            if let Some(fs) = f.as_str() {
                args.push(fs);
            }
        }
        let (stdout, stderr, success) = runner.run_with_stderr(&args);
        return if success {
            let summary = if stdout.is_empty() { stderr } else { stdout };
            Ok(json!({"success": true, "mode": "file", "target": target, "summary": summary}))
        } else {
            Err(stderr)
        };
    }

    let default_mode = "mixed";
    let mode = arguments
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or(default_mode);

    let mode_flag = match mode {
        "soft" => "--soft",
        "hard" => "--hard",
        _ => "--mixed",
    };

    let (stdout, stderr, success) = runner.run_with_stderr(&["reset", mode_flag, target]);
    if success {
        let summary = if stdout.is_empty() { stderr } else { stdout };
        Ok(json!({"success": true, "mode": mode, "target": target, "summary": summary}))
    } else {
        Err(stderr)
    }
}

#[derive(Serialize)]
struct GitMergeResult {
    success: bool,
    conflicts: Vec<String>,
    fast_forward: bool,
    summary: String,
}

fn run_merge(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("merge");

    if action == "abort" {
        let (stdout, stderr, success) = runner.run_with_stderr(&["merge", "--abort"]);
        let summary = if stdout.is_empty() { stderr } else { stdout };
        if success {
            return Ok(json!({"success": true, "summary": summary}));
        }
        return Err(summary);
    }

    let target = arguments
        .get("target")
        .and_then(|v| v.as_str())
        .ok_or("target branch/commit is required for merge")?;

    let (stdout, stderr, success) = runner.run_with_stderr(&["merge", target]);
    let summary = if stdout.is_empty() {
        stderr.clone()
    } else {
        stdout.clone()
    };

    let mut conflicts = Vec::new();
    let fast_forward = stdout.contains("Fast-forward");

    if !success {
        if stdout.contains("CONFLICT") || stderr.contains("CONFLICT") {
            let status = run_status(runner)?;
            if let Some(conflicts_arr) = status.get("conflicts").and_then(|v| v.as_array()) {
                for c in conflicts_arr {
                    if let Some(c_str) = c.as_str() {
                        conflicts.push(c_str.to_string());
                    }
                }
            }
        } else {
            return Err(summary);
        }
    }

    Ok(json!(GitMergeResult {
        success,
        conflicts,
        fast_forward,
        summary,
    }))
}

fn run_fetch(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let remote = arguments
        .get("remote")
        .and_then(|v| v.as_str())
        .unwrap_or("origin");

    let (stdout, stderr, success) = runner.run_with_stderr(&["fetch", "--prune", remote]);
    let summary = if stdout.is_empty() { stderr } else { stdout };
    if success {
        Ok(json!({"success": true, "summary": summary}))
    } else {
        Err(summary)
    }
}

fn run_tag(runner: &GitRunner, arguments: &Value) -> Result<Value, String> {
    let action = arguments
        .get("action")
        .and_then(|v| v.as_str())
        .unwrap_or("list");

    match action {
        "list" => {
            let out = runner.run(&["tag", "-l"])?;
            let tags: Vec<&str> = out
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            Ok(json!(tags))
        }
        "create" => {
            let name = arguments
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("name is required to create a tag")?;

            let message = arguments.get("message").and_then(|v| v.as_str());

            let mut args = vec!["tag"];
            if let Some(msg) = message {
                args.push("-a");
                args.push("-m");
                args.push(msg);
            }
            args.push(name);

            let (stdout, stderr, success) = runner.run_with_stderr(&args);
            if success {
                let summary = if stdout.is_empty() { stderr } else { stdout };
                Ok(json!({"success": true, "summary": summary}))
            } else {
                Err(stderr)
            }
        }
        "delete" => {
            let name = arguments
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("name is required to delete a tag")?;

            let (stdout, stderr, success) = runner.run_with_stderr(&["tag", "-d", name]);
            if success {
                let summary = if stdout.is_empty() { stderr } else { stdout };
                Ok(json!({"success": true, "summary": summary}))
            } else {
                Err(stderr)
            }
        }
        _ => Err(format!("Unknown tag action: {action}")),
    }
}
