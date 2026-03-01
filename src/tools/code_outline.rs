use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::read_to_string;
use std::path::Path;
use std::sync::OnceLock;
use tokio::task::spawn_blocking;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Symbol {
    pub r#type: String,
    pub name: String,
    pub line: usize,
    pub end_line: Option<usize>,
    pub depth: usize,
    pub children: Vec<Symbol>,
}

pub fn info() -> Value {
    json!({
        "name": "code_outline",
        "description": "Analyze a source code file and return all functions, classes, structs, enums with their line numbers. Supports Rust, Python, JavaScript, TypeScript, Go, C/C++, C#. Use this to quickly navigate large codebases.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute or relative path to the source file"
                },
                "language": {
                    "type": "string",
                    "description": "Language override. Default: auto-detect",
                    "enum": ["auto", "rust", "python", "javascript", "typescript", "go", "c", "cpp", "csharp"],
                    "default": "auto"
                },
                "include": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Symbol types to include. Default: all symbols found",
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Max nesting depth. Default: 2",
                    "default": 2
                }
            },
            "required": ["file_path"]
        }
    })
}

pub async fn run(arguments: &Value, _default_cwd: Option<&str>) -> Result<Value, String> {
    let file_path = arguments
        .get("file_path")
        .and_then(Value::as_str)
        .ok_or("file_path is required")?
        .to_string();
    let language_input = arguments
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_string();
    let max_depth = arguments
        .get("max_depth")
        .and_then(Value::as_u64)
        .map_or(2, |v| usize::try_from(v).unwrap_or(2));

    spawn_blocking(move || {
        let content =
            read_to_string(&file_path).map_err(|e| format!("Failed to read file: {e}"))?;
        let total_lines = content.lines().count();

        let language = if language_input == "auto" {
            detect_language(&file_path, &content)
        } else {
            language_input
        };

        let symbols = parse_content(&content, &language, max_depth);

        let final_output = json!({
            "file": file_path,
            "language": language,
            "total_lines": total_lines,
            "symbols": symbols
        });

        Ok(json!([{ "type": "text", "text": serde_json::to_string_pretty(&final_output).unwrap_or_default() }]))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn detect_language(path: &str, content: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust".to_string(),
        "py" => "python".to_string(),
        "js" | "mjs" | "cjs" | "jsx" => "javascript".to_string(),
        "ts" | "tsx" => "typescript".to_string(),
        "go" => "go".to_string(),
        "c" | "h" => "c".to_string(),
        "cpp" | "hpp" | "cc" | "hh" => "cpp".to_string(),
        "cs" => "csharp".to_string(),
        _ => {
            if let Some(first_line) = content.lines().next()
                && first_line.starts_with("#!")
            {
                if first_line.contains("python") {
                    return "python".to_string();
                }
                if first_line.contains("node") {
                    return "javascript".to_string();
                }
                if first_line.contains("sh") || first_line.contains("bash") {
                    return "shell".to_string();
                }
            }
            "unknown".to_string()
        }
    }
}

fn strip_string_literals(line: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut string_char = ' ';
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        i += 1;

        if !in_string && (c == '"' || c == '\'' || c == '`') {
            in_string = true;
            string_char = c;
            result.push(c);
        } else if in_string && c == '\\' && string_char != '`' {
            if i < chars.len() {
                i += 1; // skip escaped char
            }
            result.push(' ');
        } else if in_string && c == '$' && string_char == '`' {
            // Template expression ${...} - skip until }
            if i < chars.len() && chars[i] == '{' {
                i += 1; // consume {
                let mut depth = 1;
                result.push('$');
                result.push('{');
                while i < chars.len() {
                    let ic = chars[i];
                    i += 1;
                    if ic == '{' {
                        depth += 1;
                    }
                    if ic == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    result.push(' ');
                }
                result.push('}');
            } else {
                result.push(c);
            }
        } else if in_string && c == string_char {
            in_string = false;
            result.push(c);
        } else if in_string {
            result.push(' '); // replace with space to maintain length
        } else {
            result.push(c);
        }
    }
    result
}

fn raw_string_regexes() -> Option<&'static (Regex, Regex)> {
    static REGEXES: OnceLock<Option<(Regex, Regex)>> = OnceLock::new();
    REGEXES
        .get_or_init(|| {
            let re1 = Regex::new(r#"r"[^"]*""#).ok()?;
            let re2 = Regex::new(r##"r#"[^"]*"#"##).ok()?;
            Some((re1, re2))
        })
        .as_ref()
}

fn strip_rust_raw_strings(line: &str) -> String {
    let mut result = line.to_string();
    if let Some((re_raw, re_raw_hash)) = raw_string_regexes() {
        result = re_raw
            .replace_all(&result, |caps: &regex::Captures| " ".repeat(caps[0].len()))
            .to_string();
        result = re_raw_hash
            .replace_all(&result, |caps: &regex::Captures| " ".repeat(caps[0].len()))
            .to_string();
    }
    result
}

fn strip_triple_quotes(content: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = content.chars().collect();
    let mut i = 0;
    let mut in_triple = false;
    let mut triple_char = '"';

    while i < chars.len() {
        if !in_triple
            && i + 2 < chars.len()
            && (chars[i] == '"' || chars[i] == '\'')
            && chars[i + 1] == chars[i]
            && chars[i + 2] == chars[i]
        {
            in_triple = true;
            triple_char = chars[i];
            result.push(chars[i]);
            result.push(chars[i + 1]);
            result.push(chars[i + 2]);
            i += 3;
            continue;
        }

        if in_triple
            && i + 2 < chars.len()
            && chars[i] == triple_char
            && chars[i + 1] == triple_char
            && chars[i + 2] == triple_char
        {
            in_triple = false;
            result.push(chars[i]);
            result.push(chars[i + 1]);
            result.push(chars[i + 2]);
            i += 3;
            continue;
        }

        if in_triple {
            if chars[i] == '\n' {
                result.push('\n');
            } else {
                result.push(' ');
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

fn strip_block_comments(line: &str, in_comment: &mut bool) -> String {
    let mut result = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if *in_comment {
            if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '/' {
                *in_comment = false;
                i += 2;
            } else {
                i += 1;
            }
        } else if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
            *in_comment = true;
            i += 2;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn flush_stack_to_depth(stack: &mut Vec<Symbol>, symbols: &mut Vec<Symbol>, target_depth: usize) {
    while stack.last().is_some_and(|s| s.depth >= target_depth) {
        if let Some(finished) = stack.pop() {
            match stack.last_mut() {
                Some(parent) => parent.children.push(finished),
                None => symbols.push(finished),
            }
        }
    }
}

fn find_decorator_line(lines: &[&str], def_idx: usize) -> Option<usize> {
    let mut i = def_idx;
    loop {
        if i == 0 {
            break;
        }
        i -= 1;
        let trimmed = lines[i].trim();
        if trimmed.starts_with('@') {
            // Là decorator → tiếp tục lùi để tìm decorator đầu tiên
            continue;
        }

        // Blank line hoặc code thường → dừng, lùi lại 1 để i trỏ đúng
        i += 1;
        break;
    }
    // Chỉ trả Some nếu thực sự tìm thấy decorator (i < def_idx)
    if i < def_idx {
        Some(i + 1) // +1 vì line number là 1-indexed
    } else {
        None
    }
}

fn detect_python_indent(lines: &[&str]) -> usize {
    lines
        .iter()
        .filter_map(|l| {
            let trimmed = l.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let spaces = l.chars().take_while(|c| *c == ' ').count();
            if spaces > 0 { Some(spaces) } else { None }
        })
        .min()
        .unwrap_or(4)
        .max(1)
}

fn extract_symbol_name(caps: &regex::Captures) -> String {
    if caps.len() == 3 {
        // Pattern with 2 groups: class::method
        let class = caps.get(1).map_or("", |m| m.as_str());
        let method = caps.get(2).map_or("", |m| m.as_str());
        if !class.is_empty() && !method.is_empty() {
            return format!("{class}::{method}");
        }
    }
    caps.get(caps.len() - 1)
        .map_or("unknown".to_string(), |m| m.as_str().to_string())
}

fn is_valid_function_name(name: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "if",
        "else",
        "while",
        "for",
        "switch",
        "return",
        "sizeof",
        "typeof",
        "delete",
        "new",
        "case",
        "catch",
        "throw",
        // C# specific
        "using",
        "namespace",
        "lock",
        "checked",
        "unchecked",
    ];
    if KEYWORDS.contains(&name) {
        return false;
    }
    // Not a macro (all uppercase)
    if name == name.to_uppercase() && name.len() > 2 {
        return false;
    }
    true
}

struct LanguageConfig {
    patterns: Vec<(Regex, &'static str)>, // (Regex, Type)
    use_brace: bool,
}

fn get_config(language: &str) -> Result<LanguageConfig, String> {
    match language {
        "rust" => Ok(LanguageConfig {
            use_brace: true,
            patterns: vec![
                (Regex::new(r"^(?:\s*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?(?:fn)\s+([\w\d_]+)").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^(?:\s*)(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum|trait|mod)\s+([\w\d_]+)").map_err(|e| e.to_string())?, "type"),
                (Regex::new(r"^(?:\s*)impl(?:\s*<[^>]*>)?\s+([\w\d_]+(?:\s+for\s+[\w\d_]+)?)").map_err(|e| e.to_string())?, "impl"),
                (Regex::new(r"^(?:\s*)(?:pub(?:\([^)]*\))?\s+)?type\s+([\w\d_]+)").map_err(|e| e.to_string())?, "type_alias"),
            ],
        }),
        "python" => Ok(LanguageConfig {
            use_brace: false,
            patterns: vec![
                (Regex::new(r"^(?:\s*)(?:async\s+)?def\s+([\w\d_]+)").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^(?:\s*)class\s+([\w\d_]+)").map_err(|e| e.to_string())?, "class"),
            ],
        }),
        "javascript" | "typescript" => Ok(LanguageConfig {
            use_brace: true,
            patterns: vec![
                (Regex::new(r"^(?:\s*)(?:export\s+)?(?:async\s+)?function\s+([\w\d_]+)").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^(?:\s*)(?:export\s+)?class\s+([\w\d_]+)").map_err(|e| e.to_string())?, "class"),
                (Regex::new(r"^(?:\s*)(?:export\s+)?(?:interface|type|enum)\s+([\w\d_]+)").map_err(|e| e.to_string())?, "type"),
                (Regex::new(r"^(?:\s*)(?:export\s+)?const\s+([\w\d_]+)\s*=\s*(?:async\s+)?(?:\([^)]*\)|[\w\d_]+)\s*=>").map_err(|e| e.to_string())?, "arrow_function"),
            ],
        }),
        "go" => Ok(LanguageConfig {
            use_brace: true,
            patterns: vec![
                (Regex::new(r"^func\s+(?:\([^)]*\)\s+)?([\w\d_]+)").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^type\s+([\w\d_]+)\s+(?:struct|interface|enum)").map_err(|e| e.to_string())?, "type"),
            ],
        }),
        "c" | "cpp" => Ok(LanguageConfig {
            use_brace: true,
            patterns: vec![
                (Regex::new(r"^(?:class|struct|enum(?:\s+class)?|namespace|union)\s+([\w\d_]+)").map_err(|e| e.to_string())?, "type"),
                (Regex::new(r"^(?:[\w\d_*&:<>]+\s+)+([\w\d_:~]+)\s*\([^;]*$").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^\s*([\w\d_]+)::([\w\d_~]+)\s*\(").map_err(|e| e.to_string())?, "function"),
            ],
        }),
        "csharp" => Ok(LanguageConfig {
            use_brace: true,
            patterns: vec![
                (Regex::new(r"^(?:\s*)(?:(?:public|private|protected|internal|abstract|sealed|static|partial|readonly)\s+)*(?:class|struct|interface|enum|record|namespace)\s+([\w\d_]+)").map_err(|e| e.to_string())?, "type"),
                (Regex::new(r"^(?:\s*)(?:(?:public|private|protected|internal|abstract|sealed|virtual|override|static|async|extern|unsafe)\s+)*(?:[\w\d_<>,\]\[]+\s+)+([\w\d_]+)\s*\(").map_err(|e| e.to_string())?, "function"),
                (Regex::new(r"^(?:\s*)(?:(?:public|private|protected|internal|static|virtual|override|abstract|new)\s+)*[\w\d_<>\[\]]+\s+([\w\d_]+)\s*(?:\{|=>)").map_err(|e| e.to_string())?, "property"),
                (Regex::new(r"^(?:\s*)(?:(?:public|private|protected|internal|static|virtual|override)\s+)*event\s+[\w\d_<>]+\s+([\w\d_]+)").map_err(|e| e.to_string())?, "event"),
            ],
        }),
        _ => Err(format!("Unsupported language: {language}")),
    }
}

fn parse_content(content: &str, language: &str, max_depth_limit: usize) -> Vec<Symbol> {
    let Ok(config) = get_config(language) else {
        return vec![];
    };

    let preprocessed_content;
    let lines: Vec<&str> = if language == "python" {
        preprocessed_content = strip_triple_quotes(content);
        preprocessed_content.lines().collect()
    } else {
        content.lines().collect()
    };
    let mut symbols = Vec::new();
    let mut stack: Vec<Symbol> = Vec::new();
    let mut brace_depth = 0;
    let mut in_block_comment = false;

    let python_indent_size = if config.use_brace {
        4
    } else {
        detect_python_indent(&lines)
    };

    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;

        // Step 1: Strip raw strings (Rust) and regular strings
        let line_preprocessed = if language == "rust" {
            strip_rust_raw_strings(line)
        } else {
            line.to_string()
        };
        let stripped_strings = strip_string_literals(&line_preprocessed);

        // Step 2: Handle block comments
        let line_without_block = if config.use_brace && language != "python" {
            strip_block_comments(&stripped_strings, &mut in_block_comment)
        } else {
            stripped_strings
        };

        // Step 3: Strip inline comments
        let clean_line = line_without_block
            .split("//")
            .next()
            .unwrap_or("")
            .to_string();

        // Step 4: Update brace depth BEFORE everything else, but save previous depth
        let depth_before_brace = brace_depth;
        if config.use_brace {
            // clean_line is already stripped of string literals and comments
            let open = i32::try_from(clean_line.matches('{').count()).unwrap_or(0);
            let close = i32::try_from(clean_line.matches('}').count()).unwrap_or(0);
            brace_depth += open - close;
        }

        // Step 5: Check for empty line AFTER updating brace_depth
        if clean_line.trim().is_empty() {
            continue;
        }

        // Step 6: Calculate current depth for symbol
        let current_depth_i32: i32 = if config.use_brace {
            depth_before_brace
        } else {
            let leading_spaces = line.chars().take_while(|c| c.is_whitespace()).count();
            i32::try_from(leading_spaces / python_indent_size).unwrap_or(0)
        };
        let current_depth = usize::try_from(current_depth_i32).unwrap_or(0);

        for (re, sym_type) in &config.patterns {
            if let Some(caps) = re.captures(&clean_line) {
                let name = extract_symbol_name(&caps);

                // Post-match validation for C/C++/C#
                if (language == "c" || language == "cpp" || language == "csharp")
                    && !is_valid_function_name(&name)
                {
                    continue;
                }

                if current_depth > max_depth_limit {
                    continue;
                }

                let mut symbol = Symbol {
                    r#type: sym_type.to_string(),
                    name,
                    line: line_num,
                    end_line: None,
                    depth: current_depth,
                    children: Vec::new(),
                };

                // Python decorator handling
                if language == "python"
                    && let Some(dec_line) = find_decorator_line(&lines, i)
                {
                    symbol.line = dec_line;
                }

                // Approximate end_line
                if config.use_brace {
                    symbol.end_line =
                        estimate_end_line_brace(&lines, i, language, depth_before_brace);
                } else {
                    symbol.end_line = Some(estimate_end_line_indent(&lines, i));
                }

                // Add to tree
                flush_stack_to_depth(&mut stack, &mut symbols, current_depth);

                stack.push(symbol);
                break;
            }
        }
    }

    // Final stack flush
    flush_stack_to_depth(&mut stack, &mut symbols, 0);

    symbols
}

fn estimate_end_line_brace(
    lines: &[&str],
    start_idx: usize,
    language: &str,
    initial_brace_depth: i32,
) -> Option<usize> {
    let mut depth = initial_brace_depth;
    let mut found_open = false;
    let mut in_block_comment = false;

    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        let line_preprocessed = if language == "rust" {
            strip_rust_raw_strings(line)
        } else {
            line.to_string()
        };
        let stripped = strip_string_literals(&line_preprocessed);
        let clean = strip_block_comments(&stripped, &mut in_block_comment);
        let clean = clean.split("//").next().unwrap_or("");
        let open = clean.matches('{').count();
        let close = clean.matches('}').count();

        if open > 0 {
            found_open = true;
        }
        depth += i32::try_from(open).unwrap_or(0) - i32::try_from(close).unwrap_or(0);

        if found_open && depth <= initial_brace_depth {
            return Some(i + 1);
        }
    }
    None
}

fn estimate_end_line_indent(lines: &[&str], start_idx: usize) -> usize {
    let first_line_indent = lines[start_idx]
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    let mut last_valid = start_idx;

    for (i, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let current_indent = line.chars().take_while(|c| c.is_whitespace()).count();
        if current_indent <= first_line_indent {
            break;
        }
        last_valid = i;
    }
    last_valid + 1
}