//! JSONC reader (comments + trailing commas) shared by MCP / editor LLM scanners.

use std::fs;
use std::path::Path;

use serde_json::Value;

pub(crate) fn read_jsonc(path: &Path) -> Result<Option<Value>, String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let json = strip_jsonc(&text);
    serde_json::from_str(&json)
        .map(Some)
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

pub(crate) fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for nc in chars.by_ref() {
                if nc == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for nc in chars.by_ref() {
                if prev == '*' && nc == '/' {
                    break;
                }
                prev = nc;
            }
            continue;
        }
        out.push(c);
    }
    remove_trailing_commas(&out)
}

fn remove_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == ',' {
            let mut look = chars.clone();
            while matches!(look.peek(), Some(ch) if ch.is_whitespace()) {
                look.next();
            }
            if matches!(look.peek(), Some('}' | ']')) {
                continue;
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comments_and_trailing_commas() {
        let raw = r#"{
          // comment
          "a": 1,
          "b": [2, 3,],
        }"#;
        let cleaned = strip_jsonc(raw);
        let v: Value = serde_json::from_str(&cleaned).unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"][1], 3);
    }
}
