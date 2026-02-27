# Omnicommand (omnicommand-rs)

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
- **Hybrid CLI Mode**: Runs as an MCP Server or as a standalone Command Line Interface (CLI) tool.
- **Security**: Integrated blacklist to prevent dangerous commands (`rm -rf`, `format`, etc.).

## 🛠 System Requirements

- **OS**: Windows, macOS, or Linux.
- **Rust**: Latest version.
- **Make**: For automatic installation.

## 📥 Installation & Setup

### Step 1: Automatic Installation

```bash
make install
```

### Step 2: Add to PATH

Add the installation directory to your PATH to use the `omnicommand` command everywhere:

- **Windows**: `%USERPROFILE%\.omnicommand\bin`
- **Linux/macOS**: `~/.omnicommand/bin`

## ⚙️ MCP Configuration (Claude Desktop)

Configure in your `claude_desktop_config.json` file:

```json
{
  "mcpServers": {
    "omnicommand": {
      "command": "omnicommand",
      "args": []
    }
  }
}
```

## 🖥 Standalone CLI Mode (New)

You can use `omnicommand` directly from the terminal without an MCP Client:

```powershell
# Example: Run dev server in background and log to file
omnicommand run_command --command "bun run dev" --background true --logFile "dev_server.log"

# Example: Kill a process natively by name
omnicommand process_kill --name "bun"

# Check version
omnicommand --version
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
  omnicommand find_file --path "./src" --content "FIXME" --match_per_line true
  ```
- **Search for Rust files**:
  ```bash
  omnicommand find_file --path "C:\my_project" --pattern "*.rs"
  ```
- **List json files in src (dirs first)**:
  ```bash
  omnicommand list_directory --path "./src" --pattern "*.json" --dirs_first true
  ```

## ⚠️ Security Notes

- The system rejects commands in the blacklist (`rm -rf`, `format`, etc.).
- Always review and confirm AI-suggested commands before execution.

## 📄 License

This project is released under the MIT License.
