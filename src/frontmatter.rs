//! Frontmatter: a deliberately tiny `key: value` subset (plan F4).
//!
//! Hand-written because the profile is much narrower than YAML and "anything else is an error".

use crate::error::{ErrorCode, HideoError};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub hero: Option<String>,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub lang: Option<String>,
    pub hero_alt: Option<String>,
    /// Number of lines occupied by the frontmatter including both `---` fences, so that a
    /// 0-based body line index maps to a file line by `body_line_offset + index + 1`.
    pub body_line_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterError {
    pub message: String,
    pub line: usize,
}

impl std::fmt::Display for FrontmatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for FrontmatterError {}

impl Frontmatter {
    /// `title` and `hero` are required (F4). Returns one error per missing key.
    pub fn validate(&self, page: &str) -> Vec<HideoError> {
        let mut errors = Vec::new();
        for (key, value) in [("title", &self.title), ("hero", &self.hero)] {
            if value.as_deref().map(str::trim).unwrap_or("").is_empty() {
                errors.push(
                    HideoError::new(
                        ErrorCode::E001,
                        format!("missing required frontmatter key `{key}`"),
                    )
                    .with_location(page, 1),
                );
            }
        }
        errors
    }
}

/// Strip a UTF-8 BOM and NUL characters, and normalise `\r\n` and lone `\r` to `\n`.
///
/// Done once when a file is read, so that every later byte offset refers to this text.
pub fn normalize_text(raw: &str) -> String {
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\0' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push('\n');
            }
            c => out.push(c),
        }
    }
    out
}

/// Split normalised text into frontmatter and body.
pub fn parse(content: &str) -> Result<(Frontmatter, String), FrontmatterError> {
    let content = normalize_text(content);
    let lines: Vec<&str> = content.split('\n').collect();

    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return Err(FrontmatterError {
            message: "page must start with a frontmatter block opened by `---`".to_string(),
            line: 1,
        });
    }

    let Some(close) = lines
        .iter()
        .skip(1)
        .position(|l| l.trim_end() == "---")
        .map(|i| i + 1)
    else {
        return Err(FrontmatterError {
            message: "frontmatter is opened with `---` but never closed".to_string(),
            line: 1,
        });
    };

    let mut fm = Frontmatter {
        body_line_offset: close + 1,
        ..Default::default()
    };
    let mut seen: Vec<&str> = Vec::new();

    for line in &lines[1..close] {
        // Indented lines, blanks and comments are ignored.
        if line.trim().is_empty() || line.starts_with(char::is_whitespace) || line.starts_with('#')
        {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if seen.contains(&key) {
            continue; // duplicate keys: first wins
        }
        seen.push(key);
        let value = unquote(value.trim());
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

    // `split('\n')` gives a trailing empty element for a final newline; keep the body as-is
    // so byte offsets line up with a plain re-join.
    let body = lines[close + 1..].join("\n");
    Ok((fm, body))
}

/// Remove matching surrounding quotes. `\"` inside double quotes and `\'` inside single
/// quotes are unescaped (so are `\\`).
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 {
        let q = bytes[0];
        if (q == b'"' || q == b'\'') && bytes[bytes.len() - 1] == q {
            let inner = &value[1..value.len() - 1];
            let mut out = String::with_capacity(inner.len());
            let mut chars = inner.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '\\'
                    && let Some(&next) = chars.peek()
                    && (next == q as char || next == '\\')
                {
                    out.push(next);
                    chars.next();
                    continue;
                }
                out.push(c);
            }
            return out;
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fm(text: &str) -> Frontmatter {
        parse(text).unwrap().0
    }

    #[test]
    fn test_frontmatter_basic() {
        let (f, body) = parse("---\ntitle: Hello\nhero: a.png\nsubtitle: Sub\nauthor: Me\nlang: fi\nhero_alt: Alt\n---\n# Body\n").unwrap();
        assert_eq!(f.title.as_deref(), Some("Hello"));
        assert_eq!(f.hero.as_deref(), Some("a.png"));
        assert_eq!(f.subtitle.as_deref(), Some("Sub"));
        assert_eq!(f.author.as_deref(), Some("Me"));
        assert_eq!(f.lang.as_deref(), Some("fi"));
        assert_eq!(f.hero_alt.as_deref(), Some("Alt"));
        assert_eq!(body, "# Body\n");
    }

    #[test]
    fn test_frontmatter_quotes_stripped() {
        let f = fm("---\ntitle: \"Quoted\"\nhero: 'single'\n---\n");
        assert_eq!(f.title.as_deref(), Some("Quoted"));
        assert_eq!(f.hero.as_deref(), Some("single"));
    }

    #[test]
    fn test_frontmatter_duplicate_key_first_wins() {
        let f = fm("---\ntitle: One\ntitle: Two\n---\n");
        assert_eq!(f.title.as_deref(), Some("One"));
    }

    #[test]
    fn test_frontmatter_indented_lines_ignored() {
        let f = fm("---\ntitle: One\n  subtitle: nope\n\thero: nope\n---\n");
        assert_eq!(f.subtitle, None);
        assert_eq!(f.hero, None);
    }

    #[test]
    fn test_frontmatter_crlf() {
        let (f, body) = parse("---\r\ntitle: A\r\nhero: b\r\n---\r\nline1\r\nline2\r\n").unwrap();
        assert_eq!(f.title.as_deref(), Some("A"));
        assert_eq!(f.hero.as_deref(), Some("b"));
        assert_eq!(body, "line1\nline2\n");
    }

    #[test]
    fn test_frontmatter_lone_cr() {
        let (f, body) = parse("---\rtitle: A\rhero: b\r---\rbody\r").unwrap();
        assert_eq!(f.title.as_deref(), Some("A"));
        assert_eq!(body, "body\n");
    }

    #[test]
    fn test_frontmatter_missing_title_errors() {
        let f = fm("---\nhero: a.png\n---\n");
        let errors = f.validate("required/a.md");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::E001);
        assert!(errors[0].message.contains("title"));
        assert_eq!(errors[0].page.as_deref(), Some("required/a.md"));
        assert_eq!(errors[0].line, Some(1));
    }

    #[test]
    fn test_frontmatter_missing_hero_errors() {
        let f = fm("---\ntitle: T\n---\n");
        let errors = f.validate("p.md");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("hero"));
        assert!(fm("---\ntitle: T\nhero:   \n---\n").validate("p.md").len() == 1);
    }

    #[test]
    fn test_frontmatter_utf8_bom_stripped() {
        let (f, _) = parse("\u{FEFF}---\ntitle: T\nhero: h\n---\nx").unwrap();
        assert_eq!(f.title.as_deref(), Some("T"));
    }

    #[test]
    fn test_frontmatter_colon_in_value() {
        let f = fm("---\ntitle: Part 1: The start\nhero: https://x.example/a.png\n---\n");
        assert_eq!(f.title.as_deref(), Some("Part 1: The start"));
        assert_eq!(f.hero.as_deref(), Some("https://x.example/a.png"));
    }

    #[test]
    fn test_frontmatter_escaped_quotes() {
        let f = fm("---\ntitle: \"Say \\\"hi\\\"\"\nsubtitle: 'it\\'s'\n---\n");
        assert_eq!(f.title.as_deref(), Some("Say \"hi\""));
        assert_eq!(f.subtitle.as_deref(), Some("it's"));
    }

    #[test]
    fn test_frontmatter_unclosed_errors_line_1() {
        let e = parse("---\ntitle: T\nno end").unwrap_err();
        assert_eq!(e.line, 1);
        assert!(e.message.contains("never closed"));
    }

    #[test]
    fn test_frontmatter_must_open_on_first_line() {
        let e = parse("\n---\ntitle: T\n---\n").unwrap_err();
        assert_eq!(e.line, 1);
    }

    #[test]
    fn test_body_line_numbers_include_frontmatter_offset() {
        let (f, body) = parse("---\ntitle: T\nhero: h\n---\nfirst\nsecond\n").unwrap();
        assert_eq!(f.body_line_offset, 4);
        assert_eq!(body.lines().next(), Some("first"));
        // body line index 0 is file line 5
        assert_eq!(f.body_line_offset + 1, 5);
    }

    #[test]
    fn test_nul_stripped() {
        assert_eq!(normalize_text("a\0b"), "ab");
    }

    #[test]
    fn test_second_dashes_block_in_body_is_body() {
        let (_, body) = parse("---\ntitle: T\nhero: h\n---\ntext\n\n---\n\nmore\n").unwrap();
        assert!(body.contains("---"));
    }
}
