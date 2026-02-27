use glob::Pattern;
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub fn info() -> Value {
    json!({
        "name": "find_file",
        "description": "Find files based on names, content (literal/regex), size, and more. Respects .gitignore by default.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to start search" },
                "pattern": { "type": "string", "description": "Filename pattern (substring, glob, or regex)" },
                "is_regex_name": { "type": "boolean", "description": "Use regex for filename pattern", "default": false },
                "content": { "type": "string", "description": "Text content to search for inside files" },
                "is_regex": { "type": "boolean", "description": "Use regex for content search", "default": false },
                "case_sensitive": { "type": "boolean", "description": "Case sensitive matching (both name and content)", "default": false },
                "match_per_line": { "type": "boolean", "description": "If true, returns each line that matches the content query as a separate result.", "default": false },
                "extension": { "type": "string", "description": "Filter by file extension (e.g., 'rs')" },
                "min_size": { "type": "number", "description": "Minimum file size in bytes" },
                "max_size": { "type": "number", "description": "Maximum file size in bytes" },
                "limit": { "type": "number", "description": "Maximum number of results to return", "default": 100 },
                "use_gitignore": { "type": "boolean", "description": "Respect .gitignore rules", "default": true },
                "show_hidden": { "type": "boolean", "description": "Include hidden files and directories in search. Default: false.", "default": false }
            },
            "required": ["path"]
        }
    })
}

#[derive(serde::Serialize)]
pub struct ContentMatch {
    pub line_number: usize,
    pub line_content: String,
}

#[derive(serde::Serialize)]
struct FileResult {
    path: String,
    size: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    matches: Vec<ContentMatch>,
}

#[derive(serde::Serialize)]
struct FlatMatch {
    path: String,
    line_number: usize,
    line_content: String,
}

struct SearchOptions<'a> {
    pattern: Option<&'a str>,
    pattern_lower: Option<String>,
    name_regex: Option<Regex>,
    name_glob: Option<Pattern>,
    content: Option<&'a str>,
    content_lower: Option<String>,
    content_regex: Option<Regex>,
    match_per_line: bool,
    extension: Option<&'a str>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    limit: usize,
    case_sensitive: bool,
}

fn is_likely_binary(path: &Path) -> bool {
    // Check by extension first (fast path)
    let binary_exts = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "exe", "dll", "so", "dylib", "bin",
        "zip", "gz", "tar", "rar", "7z", "pdf", "wasm", "class", "pyc", "mp3", "mp4", "avi", "mov",
        "mkv", "ttf", "otf", "woff", "woff2",
    ];
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| binary_exts.contains(&ext.to_lowercase().as_str()))
    {
        return true;
    }

    // Fallback: read first 512 bytes and check for null bytes
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut buffer = [0u8; 512];
    let Ok(n) = Read::read(&mut file, &mut buffer) else {
        return false;
    };
    buffer[..n].contains(&0u8)
}

fn search_file_content(
    path: &Path,
    content: &str,
    content_lower: Option<&str>,
    content_regex: Option<&Regex>,
    case_sensitive: bool,
) -> Vec<ContentMatch> {
    let mut file_matches = Vec::new();
    let Ok(file) = fs::File::open(path) else {
        return file_matches;
    };
    let reader = BufReader::new(file);

    for (index, line_res) in reader.lines().enumerate() {
        let Ok(line_text) = line_res else {
            break;
        };

        let is_match = match content_regex {
            Some(re) => re.is_match(&line_text),
            None => {
                if case_sensitive {
                    line_text.contains(content)
                } else {
                    line_text
                        .to_lowercase()
                        .contains(content_lower.unwrap_or(content))
                }
            }
        };

        if is_match {
            file_matches.push(ContentMatch {
                line_number: index + 1,
                line_content: line_text.trim().to_string(),
            });
        }
    }
    file_matches
}

fn matches_filters(path: &Path, size: u64, options: &SearchOptions<'_>) -> bool {
    if options.min_size.is_some_and(|min| size < min) {
        return false;
    }
    if options.max_size.is_some_and(|max| size > max) {
        return false;
    }

    if options
        .extension
        .is_some_and(|ext| path.extension().and_then(|e| e.to_str()) != Some(ext))
    {
        return false;
    }

    if let Some(p) = options.pattern {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name_matches = if let Some(re) = &options.name_regex {
            re.is_match(filename)
        } else if let Some(glob) = &options.name_glob {
            glob.matches_with(
                filename,
                glob::MatchOptions {
                    case_sensitive: options.case_sensitive,
                    ..Default::default()
                },
            )
        } else if options.case_sensitive {
            filename.contains(p)
        } else {
            filename
                .to_lowercase()
                .contains(options.pattern_lower.as_deref().unwrap_or(p))
        };

        if !name_matches {
            return false;
        }
    }

    true
}

fn parse_options(arguments: &Value) -> Result<SearchOptions<'_>, String> {
    let pattern = arguments.get("pattern").and_then(Value::as_str);
    let content = arguments.get("content").and_then(Value::as_str);
    let case_sensitive = arguments
        .get("case_sensitive")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let pattern_lower = if case_sensitive {
        None
    } else {
        pattern.map(str::to_lowercase)
    };

    let mut name_regex = None;
    let mut name_glob = None;

    if let Some(p) = pattern {
        if arguments
            .get("is_regex_name")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let re_str = if case_sensitive {
                p.to_string()
            } else {
                format!("(?i){p}")
            };
            name_regex = Some(Regex::new(&re_str).map_err(|e| format!("Invalid name regex: {e}"))?);
        } else if p.contains(['*', '?', '[']) {
            name_glob = Some(Pattern::new(p).map_err(|e| format!("Invalid glob pattern: {e}"))?);
        }
    }

    let mut content_regex = None;
    let is_regex = arguments
        .get("is_regex")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let content_lower = if case_sensitive {
        None
    } else {
        content.map(str::to_lowercase)
    };
    if let (Some(c), true) = (content, is_regex) {
        let re_str = if case_sensitive {
            c.to_string()
        } else {
            format!("(?i){c}")
        };
        content_regex =
            Some(Regex::new(&re_str).map_err(|e| format!("Invalid content regex: {e}"))?);
    }

    Ok(SearchOptions {
        pattern,
        pattern_lower,
        name_regex,
        name_glob,
        content,
        content_lower,
        content_regex,
        match_per_line: arguments
            .get("match_per_line")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        extension: arguments.get("extension").and_then(Value::as_str),
        min_size: arguments.get("min_size").and_then(Value::as_u64),
        max_size: arguments.get("max_size").and_then(Value::as_u64),
        limit: arguments
            .get("limit")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(100),
        case_sensitive,
    })
}

pub fn run(arguments: &Value) -> Result<Value, String> {
    let path_str = arguments
        .get("path")
        .and_then(Value::as_str)
        .ok_or("Path is required")?;
    let options = parse_options(arguments)?;
    let use_gitignore = arguments
        .get("use_gitignore")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let show_hidden = arguments
        .get("show_hidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut results = Vec::new();
    let mut flat_results = Vec::new();
    let mut truncated = false;

    if !Path::new(path_str).exists() {
        return Err(format!("Path does not exist: {path_str}"));
    }

    for entry in WalkBuilder::new(path_str)
        .git_ignore(use_gitignore)
        .hidden(!show_hidden)
        .build()
        .flatten()
    {
        if (options.match_per_line && flat_results.len() >= options.limit)
            || (!options.match_per_line && results.len() >= options.limit)
        {
            truncated = true;
            break;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let size = metadata.len();

        if !matches_filters(path, size, &options) {
            continue;
        }

        let mut file_matches = Vec::new();
        if let Some(c) = options.content {
            if is_likely_binary(path) {
                continue;
            }
            file_matches = search_file_content(
                path,
                c,
                options.content_lower.as_deref(),
                options.content_regex.as_ref(),
                options.case_sensitive,
            );
            if file_matches.is_empty() {
                continue;
            }
        }

        if options.match_per_line {
            for m in file_matches {
                flat_results.push(FlatMatch {
                    path: path.display().to_string(),
                    line_number: m.line_number,
                    line_content: m.line_content,
                });
                if flat_results.len() >= options.limit {
                    truncated = true;
                    break;
                }
            }
            if truncated {
                break;
            }
        } else {
            results.push(FileResult {
                path: path.display().to_string(),
                size,
                matches: file_matches,
            });
        }
    }

    let final_output = if options.match_per_line {
        json!({
            "results": flat_results,
            "truncated": truncated,
            "limit": options.limit
        })
    } else {
        json!({
            "results": results,
            "truncated": truncated,
            "limit": options.limit
        })
    };

    Ok(
        json!([{ "type": "text", "text": serde_json::to_string_pretty(&final_output).unwrap_or_default() }]),
    )
}
