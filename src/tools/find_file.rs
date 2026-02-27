use glob::Pattern;
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
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
                "extension": { "type": "string", "description": "Filter by file extension (e.g., 'rs')" },
                "min_size": { "type": "number", "description": "Minimum file size in bytes" },
                "max_size": { "type": "number", "description": "Maximum file size in bytes" },
                "limit": { "type": "number", "description": "Maximum number of results to return", "default": 100 },
                "use_gitignore": { "type": "boolean", "description": "Respect .gitignore rules", "default": true }
            },
            "required": ["path"]
        }
    })
}

#[derive(serde::Serialize)]
struct ContentMatch {
    line: usize,
    text: String,
}

#[derive(serde::Serialize)]
struct FileResult {
    path: String,
    size: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    matches: Vec<ContentMatch>,
}

struct SearchOptions<'a> {
    pattern: Option<&'a str>,
    name_regex: Option<Regex>,
    name_glob: Option<Pattern>,
    content: Option<&'a str>,
    content_regex: Option<Regex>,
    extension: Option<&'a str>,
    min_size: Option<u64>,
    max_size: Option<u64>,
    limit: usize,
    case_sensitive: bool,
}

fn search_file_content(
    path: &Path,
    content: &str,
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
                    line_text.to_lowercase().contains(&content.to_lowercase())
                }
            }
        };

        if is_match {
            file_matches.push(ContentMatch {
                line: index + 1,
                text: line_text.trim().to_string(),
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
            filename.to_lowercase().contains(&p.to_lowercase())
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
        name_regex,
        name_glob,
        content,
        content_regex,
        extension: arguments.get("extension").and_then(Value::as_str),
        min_size: arguments.get("min_size").and_then(Value::as_u64),
        max_size: arguments.get("max_size").and_then(Value::as_u64),
        limit: arguments
            .get("limit")
            .and_then(|l| {
                #[allow(clippy::cast_possible_truncation)]
                l.as_u64().map(|v| v as usize)
            })
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
    let mut results = Vec::new();

    if !Path::new(path_str).exists() {
        return Err(format!("Path does not exist: {path_str}"));
    }

    for entry in WalkBuilder::new(path_str)
        .git_ignore(use_gitignore)
        .hidden(false)
        .build()
        .flatten()
    {
        if results.len() >= options.limit {
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
            file_matches = search_file_content(
                path,
                c,
                options.content_regex.as_ref(),
                options.case_sensitive,
            );
            if file_matches.is_empty() {
                continue;
            }
        }

        results.push(FileResult {
            path: path.display().to_string(),
            size,
            matches: file_matches,
        });
    }

    Ok(
        json!([{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap_or_default() }]),
    )
}
