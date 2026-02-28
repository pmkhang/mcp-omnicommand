# Omni (omni-rs)

[Vietnamese Version](./README.md)

A powerful MCP (Model Context Protocol) server written in Rust, allowing AI models (like Claude) to interact directly with your operating system via the command line. Fully supports Windows, macOS, and Linux.

## 🚀 Key Features

- **Smart Shell Execution (`run_command`)**:
  - Automatically detects OS and default Shell (CMD on Windows, Sh on Unix).
  - Supports **Background** execution (non-blocking) for long-running tasks (like servers).
  - **Real-time Log Tracking**: Redirects logs directly to a file when running in the background.
  - Supports single or batch execution (Parallel/Sequential).
- **Native Process Management**:
  - List (`process_list`), cleanup (`process_cleanup`), and **Kill process (`process_kill`)** using native code (no dependency on system commands).
- **Advanced File & Directory Operations**:
  - `list_directory`: Intelligent directory listing with `.gitignore` support, sorting by size/date, and directory grouping.
  - `find_file`: Powerful file searching using Regex, Glob Patterns (`*.rs`), or text content.
  - `tail_file`: Read the last N lines of a file (perfect for log monitoring).
- **Sync & Wait (`wait_for`)**:
  - Wait for specific conditions: File existence, Port reachability, or Process termination.
- **Network Communication (`fetch_api`)**:
  - Make HTTP requests (GET, POST, etc.) directly from the command line like `curl`.
- **Hybrid CLI Mode**: Runs as an MCP Server or as a standalone Command Line Interface (CLI) tool.
- **Security**: Integrated blacklist to prevent dangerous commands (`rm -rf`, `format`, etc.).

## 🛠 System Requirements

- **OS**: Windows, macOS, or Linux.
- **Rust**: Latest version.
- **Make**: For automatic installation.

## 📥 Installation & Setup

### Step 1: Automatic Installation

If this is your **first time installing**, run this command to install and automatically add the omni directory to your PATH environment variable:

```bash
make first-install
```

(_Note: Once completed, you will see a success message. You must restart your terminal for the `omni` command to be recognized._)

If you just want to reinstall or update to a new version:

```bash
make install
```

### Step 2: Add to PATH (Manual, if automatic fails)

Add the installation directory to your PATH to use the `omni` command everywhere:

- **Windows**: `%USERPROFILE%\.omni\bin`
- **Linux/macOS**: `~/.omni/bin`

## ⚙️ MCP Configuration (Claude Desktop)

Configure in your `claude_desktop_config.json` file:

```json
{
  "mcpServers": {
    "omni": {
      "command": "omni",
      "args": ["@mcp"]
    }
  }
}
```

## 🖥 Standalone CLI Mode (New)

You can use `omni` directly from the terminal without an MCP Client:

```powershell
# Example: Run dev server in background and log to file
omni run_command --command "bun run dev" --background true --logFile "dev_server.log"

# Example: Kill a process natively by name
omni process_kill --name "bun"

# Check version
omni --version
```

## 🛠 Available Tools

| Tool              | Description                         | Key Parameters                                             |
| :---------------- | :---------------------------------- | :--------------------------------------------------------- |
| `run_command`     | Run one or more shell commands.     | `command`, `background`, `logFile`, `shell`, `runParallel` |
| `process_list`    | List running processes.             | `filter`                                                   |
| `process_kill`    | Kill a process by PID or name.      | `pid`, `name`, `force`                                     |
| `process_cleanup` | Clean up hanging shell processes.   | `maxAgeSeconds`, `dryRun`, `includeNode`                   |
| `list_directory`  | List directory context (gitignore). | `path`, `max_depth`, `dirs_first`, `pattern`               |
| `find_file`       | Find files by name, regex, content  | `path`, `pattern`, `content`, `is_regex`, `match_per_line` |
| `tail_file`       | Read last N lines of a file.        | `path`, `lines`                                            |
| `wait_for`        | Wait for Port, File, or Process.    | `strategy`, `target`, `timeout`, `interval`                |
| `fetch_api`       | Make HTTP requests (curl-like).     | `url`, `method`, `headers`, `body`, `timeout`              |

## 📖 Advanced Examples

- **Run background server**:
  ```json
  { "command": "npm run dev", "background": true, "logFile": "dev.log" }
  ```
- **Kill all hanging Node processes**:
  ```json
  { "name": "node", "force": true }
  ```
- **Find all lines with 'FIXME' (flat mode)**:
  ```bash
  omni find_file --path "./src" --content "FIXME" --match_per_line true
  ```
- **Real-time log monitoring**:
  ```bash
  omni tail_file --path "dev_server.log" --lines 20
  ```
- **Wait for server before proceeding**:
  ```bash
  omni wait_for --strategy "port" --target "127.0.0.1:8080" --timeout 60000
  ```
- **Make an HTTP request**:
  ```bash
  omni fetch_api --url "https://jsonplaceholder.typicode.com/posts/1"
  ```
- **Search for Rust files**:
  ```bash
  omni find_file --path "C:\my_project" --pattern "*.rs"
  ```
- **List json files in src (dirs first)**:
  ```bash
  omni list_directory --path "./src" --pattern "*.json" --dirs_first true
  ```

## ⚠️ Security Notes

- The system rejects commands in the blacklist (`rm -rf`, `format`, etc.).
- Always review and confirm AI-suggested commands before execution.

## 📄 License

This project is released under the MIT License.
