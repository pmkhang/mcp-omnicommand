use glob::Pattern;
use ignore::WalkBuilder;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::fs::Metadata;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub fn info() -> Value {
    json!({
        "name": "list_directory",
        "description": "Get a detailed listing of all files and directories in a specified path with advanced filtering and summaries.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to list" },
                "max_depth": { "type": "number", "description": "Maximum recursion depth (0 for current directory only)", "default": 0 },
                "sort_by": { "type": "string", "enum": ["name", "size", "modified"], "description": "Field to sort by", "default": "name" },
                "order": { "type": "string", "enum": ["asc", "desc"], "description": "Sort order", "default": "asc" },
                "dirs_first": { "type": "boolean", "description": "List directories before files", "default": true },
                "show_hidden": { "type": "boolean", "description": "Show hidden files", "default": false },
                "pattern": { "type": "string", "description": "Filter items by name pattern (substring or glob)" },
                "case_sensitive": { "type": "boolean", "description": "Case sensitive matching for pattern", "default": false },
                "use_gitignore": { "type": "boolean", "description": "Respect .gitignore rules", "default": true },
                "limit": { "type": "number", "description": "Maximum number of items to return in the list", "default": 200 }
            },
            "required": ["path"]
        }
    })
}

#[derive(serde::Serialize)]
struct EntryInfo {
    name: String,
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: u64,
    modified: u64,
}

#[derive(serde::Serialize, Default)]
struct Summary {
    files: u64,
    dirs: u64,
    size: u64,
}

struct FilterFlags {
    show_hidden: bool,
    use_gitignore: bool,
}

struct DisplayFlags {
    dirs_first: bool,
    case_sensitive: bool,
}

struct ListOptions {
    max_depth: Option<usize>,
    sort_by: String,
    order: String,
    pattern: Option<String>,
    filter: FilterFlags,
    display: DisplayFlags,
    limit: usize,
}

fn parse_args(arguments: &Value) -> ListOptions {
    let limit = arguments
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(200);

    let max_depth = arguments
        .get("max_depth")
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok());

    ListOptions {
        max_depth,
        sort_by: arguments
            .get("sort_by")
            .and_then(Value::as_str)
            .unwrap_or("name")
            .to_string(),
        order: arguments
            .get("order")
            .and_then(Value::as_str)
            .unwrap_or("asc")
            .to_string(),
        pattern: arguments
            .get("pattern")
            .and_then(Value::as_str)
            .map(String::from),
        filter: FilterFlags {
            show_hidden: arguments
                .get("show_hidden")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            use_gitignore: arguments
                .get("use_gitignore")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        },
        display: DisplayFlags {
            dirs_first: arguments
                .get("dirs_first")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            case_sensitive: arguments
                .get("case_sensitive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        limit,
    }
}

fn matches_pattern(name: &str, pattern: &str, case_sensitive: bool) -> bool {
    let is_glob = pattern.contains(['*', '?', '[']);
    if is_glob {
        let Ok(p) = Pattern::new(pattern) else {
            return false;
        };
        return p.matches_with(
            name,
            glob::MatchOptions {
                case_sensitive,
                ..Default::default()
            },
        );
    }

    if case_sensitive {
        name.contains(pattern)
    } else {
        name.to_lowercase().contains(&pattern.to_lowercase())
    }
}

fn collect_entries(path_str: &str, options: &ListOptions, summary: &mut Summary) -> Vec<EntryInfo> {
    let mut entries = Vec::new();
    let walker = WalkBuilder::new(path_str)
        .git_ignore(options.filter.use_gitignore)
        .hidden(!options.filter.show_hidden)
        .max_depth(options.max_depth)
        .build();

    for entry in walker.flatten() {
        if entry.depth() == 0 && entry.path() == Path::new(path_str) {
            continue;
        }

        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let matches = options
            .pattern
            .as_ref()
            .is_none_or(|p| matches_pattern(name, p, options.display.case_sensitive));

        if !matches {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = metadata.as_ref().is_some_and(Metadata::is_dir);
        let size = metadata.as_ref().map_or(0, Metadata::len);
        let modified = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());

        if is_dir {
            summary.dirs += 1;
        } else {
            summary.files += 1;
            summary.size += size;
        }

        entries.push(EntryInfo {
            name: name.to_string(),
            path: path.display().to_string(),
            entry_type: if is_dir { "DIR" } else { "FILE" }.to_string(),
            size,
            modified,
        });
    }
    entries
}

pub fn run(arguments: &Value) -> Result<Value, String> {
    let path_str = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or("Path is required")?;
    let options = parse_args(arguments);

    if !Path::new(path_str).exists() {
        return Err(format!("Path does not exist: {path_str}"));
    }

    let mut summary = Summary::default();
    let mut entries = collect_entries(path_str, &options, &mut summary);

    entries.sort_by(|a, b| {
        if options.display.dirs_first && a.entry_type != b.entry_type {
            if a.entry_type == "DIR" {
                return Ordering::Less;
            }
            return Ordering::Greater;
        }

        let cmp = match options.sort_by.as_str() {
            "size" => a.size.cmp(&b.size),
            "modified" => a.modified.cmp(&b.modified),
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        if options.order == "desc" {
            cmp.reverse()
        } else {
            cmp
        }
    });

    let total_found = entries.len();
    if entries.len() > options.limit {
        entries.truncate(options.limit);
    }

    Ok(json!([{
        "type": "text",
        "text": serde_json::to_string_pretty(&json!({
            "summary": summary,
            "items": entries,
            "truncated": total_found > options.limit
        })).unwrap_or_default()
    }]))
}
