#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Inline,
    BlockStart,
}

pub fn escape_text(text: &str, position: Position) -> String {
    let mut result = String::with_capacity(text.len() + 16);
    let mut chars = text.chars().peekable();
    let mut at_line_start = matches!(position, Position::BlockStart);

    while let Some(c) = chars.next() {
        if at_line_start {
            at_line_start = false;
            if c == '\n' {
                at_line_start = true;
                result.push('\n');
                continue;
            }
        }

        match c {
            '\\' => result.push_str(r"\\"),
            '#' => result.push_str(r"\#"),
            '$' => result.push_str(r"\$"),
            '@' => result.push_str(r"\@"),
            '*' => {
                if at_line_start {
                    result.push_str(r"\*");
                } else {
                    result.push_str(r"\*");
                }
            }
            '_' => {
                if at_line_start {
                    result.push_str(r"\_");
                } else {
                    result.push_str(r"\_");
                }
            }
            '`' => result.push_str(r"\`"),
            '<' => result.push_str(r"\<"),
            '>' => result.push_str(r"\>"),
            '[' => result.push_str(r"\["),
            ']' => result.push_str(r"\]"),
            '~' => result.push_str(r"\~"),
            '/' => {
                if chars.peek() == Some(&'/') {
                    chars.next();
                    result.push_str(r"\/\/");
                } else if at_line_start {
                    result.push_str(r"\/");
                } else {
                    result.push('/');
                }
            }
            '=' => {
                if at_line_start {
                    result.push_str(r"\=");
                } else {
                    result.push('=');
                }
            }
            '-' => {
                if at_line_start {
                    result.push_str(r"\-");
                } else {
                    result.push('-');
                }
            }
            '+' => {
                if at_line_start {
                    result.push_str(r"\+");
                } else {
                    result.push('+');
                }
            }
            '0'..='9' => {
                if at_line_start && chars.peek() == Some(&'.') {
                    result.push(c);
                    result.push_str(r"\.");
                } else {
                    result.push(c);
                }
            }
            '\n' => {
                result.push('\n');
                at_line_start = true;
            }
            _ => result.push(c),
        }
    }

    result
}

pub fn escape_raw_inline(content: &str) -> String {
    let backtick_count = count_backticks(content);
    let delimiter = "`".repeat(backtick_count.max(1) + 1);
    let padding = if content.starts_with('`') || content.ends_with('`') { " " } else { "" };
    format!("{}{}{}{}", delimiter, padding, content, padding)
}

pub fn escape_raw_block(content: &str, info_string: Option<&str>) -> String {
    let backtick_count = count_backticks(content);
    let delimiter = "`".repeat(backtick_count.max(3) + 1);
    let lang = info_string
        .filter(|s| is_valid_lang_tag(s))
        .map(|s| format!("{} ", s))
        .unwrap_or_default();
    format!("{}{}\n{}\n{}", delimiter, lang, content, delimiter)
}

fn count_backticks(content: &str) -> usize {
    let mut max_run = 0;
    let mut current_run = 0;
    for c in content.chars() {
        if c == '`' {
            current_run += 1;
            max_run = max_run.max(current_run);
        } else {
            current_run = 0;
        }
    }
    max_run
}

fn is_valid_lang_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '#' || c == '-' || c == '.')
}