use serde_json::{Value, json};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

pub fn info() -> Value {
    json!({
        "name": "tail_file",
        "description": "Read the last N lines of a file (useful for monitoring logs).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file" },
                "lines": { "type": "number", "description": "Number of lines to read from the end", "default": 10 }
            },
            "required": ["path"]
        }
    })
}

pub fn run(arguments: &Value) -> Result<Value, String> {
    let path = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or("Path is required")?;
    let lines_to_read_u64 = arguments.get("lines").and_then(Value::as_u64).unwrap_or(10);
    let lines_to_read = usize::try_from(lines_to_read_u64).unwrap_or(usize::MAX);

    let file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let file_size = file
        .metadata()
        .map_err(|e| format!("Failed to get metadata: {e}"))?
        .len();

    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();

    if file_size == 0 {
        return Ok(json!({ "lines": lines }));
    }

    // Heuristic: start seeking from 200 bytes per line from the end
    let bytes_to_jump = lines_to_read_u64.saturating_mul(200);
    let seek_start = file_size.saturating_sub(bytes_to_jump);

    reader
        .get_mut()
        .seek(SeekFrom::Start(seek_start))
        .map_err(|e| format!("Seek failed: {e}"))?;

    let mut all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    // Take the last N lines from what we read
    let start_idx = all_lines.len().saturating_sub(lines_to_read);
    lines = all_lines.drain(start_idx..).collect();

    Ok(json!([{
        "type": "text",
        "text": serde_json::to_string(&json!({
            "path": path,
            "lines": lines
        })).unwrap_or_default()
    }]))
}
