use std::collections::HashMap;
use crate::error::{HideoError, ErrorCode};

#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub hero: Option<String>,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub lang: Option<String>,
    pub hero_alt: Option<String>,
    pub body_line_offset: usize,
}

#[derive(Debug, Clone)]
pub struct FrontmatterError {
    pub message: String,
    pub line: Option<usize>,
}

impl std::fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FrontmatterError {}

pub fn parse(content: &str) -> Result<(Frontmatter, String), FrontmatterError> {
    let content = strip_bom(content);
    let content = normalize_line_endings(&content);

    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() || lines[0].trim() != "---" {
        return Err(FrontmatterError {
            message: "frontmatter must start with '---'".to_string(),
            line: Some(1),
        });
    }

    let mut end_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end_idx = Some(i);
            break;
        }
    }

    let end_idx = end_idx.ok_or_else(|| FrontmatterError {
        message: "frontmatter never closed with '---'".to_string(),
        line: Some(1),
    })?;

    let frontmatter_lines = &lines[1..end_idx];
    let body_lines = &lines[end_idx + 1..];

    let mut fm = Frontmatter::default();
    fm.body_line_offset = end_idx + 1;

    let mut seen_keys = HashMap::new();

    for (idx, line) in frontmatter_lines.iter().enumerate() {
        let line_num = idx + 2;
        let line = *line;
        let trimmed = line.trim_start();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed != line {
            continue;
        }

        let Some(colon_pos) = line.find(':') else {
            continue;
        };

        let key = line[..colon_pos].trim();
        let value = line[colon_pos + 1..].trim();

        if seen_keys.contains_key(key) {
            continue;
        }
        seen_keys.insert(key.to_string(), ());

        let value = unquote_value(value);

        match key {
            "title" => fm.title = Some(value),
            "hero" => fm.hero = Some(value),
            "subtitle" => fm.subtitle = Some(value),
            "author" => fm.author = Some(value),
            "lang" => fm.lang = Some(value),
            "hero_alt" => fm.hero_alt = Some(value),
            _ => {}
        }
    }

    let body = body_lines.join("\n");
    Ok((fm, body))
}

fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{FEFF}').unwrap_or(content)
}

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

fn unquote_value(value: &str) -> String {
    if value.len() >= 2 {
        let first = value.chars().next().unwrap();
        let last = value.chars().last().unwrap();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            let inner = &value[1..value.len() - 1];
            return unescape_quotes(inner, first);
        }
    }
    value.to_string()
}

fn unescape_quotes(value: &str, quote_char: char) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == quote_char || next == '\\' {
                    chars.next();
                    result.push(next);
                    continue;
                }
            }
        }
        result.push(c);
    }

    result
}