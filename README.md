# Omni (omni-rs)

[English Version](./README_EN.md)

Một máy chủ MCP (Model Context Protocol) mạnh mẽ được viết bằng Rust, cho phép các mô hình AI (như Claude) tương tác trực tiếp với hệ điều hành của bạn thông qua dòng lệnh. Dự án hỗ trợ đầy đủ Windows, macOS và Linux.

## 🚀 Tính năng chính

- **Chạy lệnh Shell Thông minh (`run_command`)**:
  - Tự động nhận diện OS và Shell mặc định (CMD trên Windows, Sh trên Unix).
  - Hỗ trợ chạy lệnh **Background** (không chặn) cho các tác vụ lâu dài (như server).
  - **Real-time Log Tracking**: Ghi log trực tiếp vào file khi chạy ngầm.
  - Hỗ trợ chạy lẻ hoặc batch (Parallel/Sequential).
- **Quản lý quy trình Native**:
  - Liệt kê (`process_list`), dọn dẹp (`process_cleanup`) và **Tắt process (`process_kill`)** bằng code native (không phụ thuộc lệnh hệ thống).
- **File & Directory Operations (Nâng cao)**:
  - `list_directory`: Liệt kê cây thư mục thông minh, hỗ trợ `.gitignore`, sắp xếp theo size/date, và nhóm thư mục lên đầu.
  - `find_file`: Tìm kiếm file cực mạnh bằng Regex, Glob Pattern (`*.rs`) hoặc nội dung text.
  - `tail_file`: Đọc N dòng cuối của file (cực kỳ hữu ích để theo dõi log).
- **Đồng bộ & Chờ đợi (`wait_for`)**:
  - Chờ đợi một điều kiện cụ thể: File xuất hiện, Port mở, hoặc Process kết thúc.
- **Giao tiếp mạng (`fetch_api`)**:
  - Thực hiện các HTTP request (GET, POST, v.v.) trực tiếp từ dòng lệnh tương tự như `curl`.
- **Git Operations Nâng cao (`git_operations`)**:
  - Hỗ trợ Git trực tiếp không cần CLI: status, log, diff, branch, add, commit, push, pull, checkout.
  - Tích hợp chuyên sâu như `stash` (lưu nháp), `reset` (unstaging), `merge` (gộp nhánh và bắt xung đột), `fetch` (tải remote ref), và `tag` (quản lý phiên bản).
- **Chế độ Hybrid CLI**: Chạy như một MCP Server hoặc như một công cụ dòng lệnh (CLI) độc lập.
- **Bảo mật**: Tích hợp danh sách đen (blacklist) ngăn chặn các lệnh nguy hiểm (rm -rf, format, v.v.).

## 🛠 Yêu cầu hệ thống

- **Hệ điều hành**: Windows, macOS hoặc Linux.
- **Rust**: Phiên bản mới nhất.
- **Make**: Để cài đặt tự động.

## 📥 Cài đặt & Thiết lập

### Bước 1: Cài đặt tự động

Nếu đây là **lần cài đặt đầu tiên**, hãy chạy lệnh này để tự động cài đặt và thêm thư mục vào biến môi trường PATH:

```bash
make first-install
```

(_Lưu ý: Sau khi chạy, bạn copy dòng báo thành công và khởi động lại Terminal để hệ thống nhận diện lệnh `omni`._)

Nếu bạn chỉ muốn cài đặt lại/cập nhật phiên bản mới:

```bash
make install
```

### Bước 2: Thêm vào PATH (Thủ công, nếu lệnh tự động thất bại)

Thêm thư mục cài đặt vào PATH để dùng lệnh `omni` ở mọi nơi:

- **Windows**: `%USERPROFILE%\.omni\bin`
- **Linux/macOS**: `~/.omni/bin`

## ⚙️ Cấu hình MCP (Claude Desktop)

Cấu hình trong file `claude_desktop_config.json`:

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

## 🖥 Chế độ CLI Độc lập (Mới)

Bạn có thể sử dụng `omni` trực tiếp từ terminal mà không cần MCP Client:

```powershell
# Ví dụ chạy server ngầm và ghi log
omni run_command --command "bun run dev" --background true --logFile "dev_server.log"

# Ví dụ tắt process native theo tên
omni process_kill --name "bun"
```

## 🛠 Các công cụ (Tools) sẵn có

| Tool              | Mô tả                                            | Tham số chính                                              |
| :---------------- | :----------------------------------------------- | :--------------------------------------------------------- |
| `run_command`     | Chạy một hoặc nhiều lệnh shell.                  | `command`, `background`, `logFile`, `shell`, `runParallel` |
| `process_list`    | Liệt kê các tiến trình đang chạy.                | `filter`                                                   |
| `process_kill`    | Tắt tiến trình bằng PID hoặc tên.                | `pid`, `name`, `force`                                     |
| `process_cleanup` | Dọn dẹp các tiến trình shell bị treo.            | `maxAgeSeconds`, `dryRun`, `includeNode`                   |
| `list_directory`  | Liệt kê thư mục (hỗ trợ gitignore).              | `path`, `max_depth`, `dirs_first`, `pattern`               |
| `find_file`       | Tìm file theo tên, regex, nội dung.              | `path`, `pattern`, `content`, `is_regex`, `match_per_line` |
| `tail_file`       | Đọc N dòng cuối của file.                        | `path`, `lines`                                            |
| `wait_for`        | Đợi Port, File, hoặc Process kết thúc.           | `strategy`, `target`, `timeout`, `interval`                |
| `fetch_api`       | Thực hiện HTTP request (curl-like).              | `url`, `method`, `headers`, `body`, `timeout`              |
| `git_operations`  | Tương tác mã nguồn Git (status, pull, merge...). | `op`, `cwd`, `target`, `message`, `files`                  |

## 📖 Ví dụ nâng cao

- **Chạy server ngầm**:
  ```json
  { "command": "npm run dev", "background": true, "logFile": "dev.log" }
  ```
- **Tắt tất cả Node process treo**:
  ```json
  { "name": "node", "force": true }
  ```
- **Tìm tất cả dòng có chữ 'FIXME' (dạng phẳng)**:
  ```bash
  omni find_file --path "./src" --content "FIXME" --match_per_line true
  ```
- **Theo dõi log real-time**:
  ```bash
  omni tail_file --path "dev_server.log" --lines 20
  ```
- **Chờ server sẵn sàng rổi mới chạy tiếp**:
  ```bash
  omni wait_for --strategy "port" --target "127.0.0.1:8080" --timeout 60000
  ```
- **Gọi HTTP API**:
  ```bash
  omni fetch_api --url "https://jsonplaceholder.typicode.com/posts/1"
  ```
- **Tìm kiếm file Rust**:
  ```bash
  omni find_file --path "C:\my_project" --pattern "*.rs"
  ```
- **Liệt kê file json trong src (dirs first)**:
  ```bash
  omni list_directory --path "./src" --pattern "*.json" --dirs_first true
  ```

## ⚠️ Lưu ý bảo mật

- Hệ thống từ chối các lệnh trong danh sách đen (`rm -rf`, `format`, v.v.).
- Luôn kiểm soát các lệnh AI đề xuất trước khi chạy.

## 📄 Giấy phép

Dự án phát hành dưới giấy phép MIT.
