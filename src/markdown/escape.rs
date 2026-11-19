//! Typst escaping (plan 7.5). The highest-risk module: every character that reaches the
//! generated Typst passes through here.

/// Where in a line a text run starts. At the start of a line (or of a content block) the
/// characters `=`, `-`, `+`, `/` and `N.` would be read as heading, list, enum or term-list
/// markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Inline,
    LineStart,
    /// Directly after an embedded expression such as `#strong[x]` or `#raw("x")`. Typst would
    /// read a following `.name` as a field access and `(` as a call (`#strong[x](y)` is an
    /// error), so those two characters must be escaped when they come first.
    AfterExpr,
}

/// Escape prose for Typst *markup* mode. The text must not contain newlines (the emitter turns
/// soft and hard breaks into separate events).
///
/// Typst's own shorthands (`...`, `--`, `---`, `-?`) and smart quotes are intentionally left
/// alone: they are what the plan wants for prose.
pub fn escape_text(text: &str, position: Position) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let at_start = i == 0 && position == Position::LineStart;
        match c {
            '\\' | '#' | '$' | '@' | '*' | '_' | '`' | '<' | '>' | '[' | ']' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '/' => {
                // `//` starts a comment (and would auto-link `https://`), `/*` a block
                // comment (its `*` is escaped anyway), a leading `/` is a term-list marker.
                if chars.get(i + 1) == Some(&'/') {
                    out.push_str("\\/\\/");
                    i += 1;
                } else if at_start {
                    out.push_str("\\/");
                } else {
                    out.push('/');
                }
            }
            // `(` would be a call and `;` a code-mode terminator that Typst swallows.
            '(' | ';' if i == 0 && position == Position::AfterExpr => {
                out.push('\\');
                out.push(c);
            }
            // `.5` and `. ` are harmless; only `.name` is a field access.
            '.' if i == 0
                && position == Position::AfterExpr
                && chars.get(1).is_some_and(|n| n.is_alphabetic() || *n == '_') =>
            {
                out.push_str("\\.");
            }
            '-' => {
                let next = chars.get(i + 1).copied();
                if next == Some('-') {
                    // `--` and `---` are the dash shorthands: keep them intact.
                    let n = if chars.get(i + 2) == Some(&'-') { 3 } else { 2 };
                    out.push_str(&"-".repeat(n));
                    i += n;
                    continue;
                } else if next == Some('?') {
                    out.push_str("-?"); // soft hyphen shorthand
                    i += 2;
                    continue;
                } else if at_start || next.is_some_and(char::is_numeric) {
                    // A list marker at line start; and `-5` would become a minus sign
                    // (U+2212), so `COVID-19` would not keep its hyphen.
                    out.push_str("\\-");
                } else {
                    out.push('-');
                }
            }
            '=' | '+' if at_start => {
                out.push('\\');
                out.push(c);
            }
            '0'..='9' if at_start => {
                // `12. item` is an enum item: keep the digits, escape the dot.
                let mut j = i;
                while j < chars.len() && chars[j].is_ascii_digit() {
                    out.push(chars[j]);
                    j += 1;
                }
                if chars.get(j) == Some(&'.') {
                    out.push_str("\\.");
                    j += 1;
                }
                // Everything after the marker is ordinary inline text.
                out.push_str(&escape_text(
                    &chars[j..].iter().collect::<String>(),
                    Position::Inline,
                ));
                return out;
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

/// A Typst string literal (with quotes) for arbitrary text. Used for URLs, raw code and alt
/// text, where nothing is interpreted as markup.
pub fn typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Language tags are passed through only if they look like one (plan 7.5).
pub fn is_valid_lang_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '#' | '.' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline(s: &str) -> String {
        escape_text(s, Position::Inline)
    }
    fn start(s: &str) -> String {
        escape_text(s, Position::LineStart)
    }

    #[test]
    fn escapes_markup_characters() {
        assert_eq!(
            inline(r"a#b$c@d*e_f`g<h>i[j]k~l\m"),
            r"a\#b\$c\@d\*e\_f\`g\<h\>i\[j\]k\~l\\m"
        );
    }

    #[test]
    fn escapes_comments_and_urls() {
        assert_eq!(inline("https://x.fi/a/b"), r"https:\/\/x.fi/a/b");
        assert_eq!(inline("a /* b */ c"), r"a /\* b \*/ c");
        assert_eq!(inline("a//b"), r"a\/\/b");
        assert_eq!(inline("1/2"), "1/2");
    }

    #[test]
    fn escapes_line_start_markers_only_at_line_start() {
        assert_eq!(start("= x"), r"\= x");
        assert_eq!(start("- x"), r"\- x");
        assert_eq!(start("+ x"), r"\+ x");
        assert_eq!(start("/ x"), r"\/ x");
        assert_eq!(start("12. x"), r"12\. x");
        assert_eq!(start("12 x"), "12 x");
        assert_eq!(start("-- x"), "-- x");
        assert_eq!(start("-5"), r"\-5");
        assert_eq!(start("1.5 x"), r"1\.5 x");
        assert_eq!(inline("= x - y + z 1. a"), "= x - y + z 1. a");
        assert_eq!(start("a = b"), "a = b");
    }

    #[test]
    fn escapes_postfix_triggers_after_an_expression() {
        let after = |s: &str| escape_text(s, Position::AfterExpr);
        assert_eq!(after(".com"), r"\.com");
        assert_eq!(after("(x)"), r"\(x)");
        assert_eq!(after(";x"), r"\;x");
        assert_eq!(inline(";x"), ";x");
        assert_eq!(after(" .com"), " .com");
        assert_eq!(after(". Next"), ". Next");
        assert_eq!(after(".5"), ".5");
        assert_eq!(after("."), ".");
        assert_eq!(after("x.com"), "x.com");
        assert_eq!(inline(".com"), ".com");
    }

    #[test]
    fn hyphen_before_digit_stays_a_hyphen() {
        assert_eq!(
            inline("COVID-19 and a-b and 2020-2021"),
            r"COVID\-19 and a-b and 2020\-2021"
        );
        assert_eq!(inline("a--5"), "a--5");
        assert_eq!(inline("a-?b"), "a-?b");
    }

    #[test]
    fn leaves_shorthands_and_quotes_for_typst() {
        assert_eq!(inline("a -- b --- c ... d -? e"), "a -- b --- c ... d -? e");
        assert_eq!(inline("\"quoted\" and 'single'"), "\"quoted\" and 'single'");
    }

    #[test]
    fn zero_width_space_is_never_escaped() {
        assert_eq!(inline("a.\u{200B}b"), "a.\u{200B}b");
    }

    #[test]
    fn string_literal_escapes() {
        assert_eq!(typst_string("a\"b\\c\n\rd\te"), r#""a\"b\\c\n\rd\te""#);
        assert_eq!(typst_string("\u{1}"), r#""\u{1}""#);
    }

    #[test]
    fn lang_tags() {
        assert!(is_valid_lang_tag("rust"));
        assert!(is_valid_lang_tag("c++"));
        assert!(is_valid_lang_tag("c#"));
        assert!(!is_valid_lang_tag(""));
        assert!(!is_valid_lang_tag("a b"));
        assert!(!is_valid_lang_tag("a\"b"));
    }
}

/// Property tests: escaped text must compile in real Typst and its text must survive
/// (plan 7.5 "Property test", 8.3).
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::compile::{compile_document, extract_text};
    use crate::world::HideoWorld;
    use proptest::prelude::*;
    use typst::foundations::Datetime;

    fn compile_text(src: &str) -> Result<String, String> {
        let world = HideoWorld::from_source(src, Datetime::from_ymd(2000, 1, 1).unwrap());
        let warned = compile_document(&world);
        if !warned.warnings.is_empty() {
            return Err(format!("warnings: {:?}", warned.warnings));
        }
        match warned.output {
            Ok(doc) => Ok(extract_text(&doc).concat()),
            Err(e) => Err(format!(
                "{:?}",
                e.iter().map(|d| d.message.to_string()).collect::<Vec<_>>()
            )),
        }
    }

    /// What Typst's markup turns prose into: shorthands applied, quotes and whitespace removed
    /// so the comparison is about *which characters survive*.
    fn expected(s: &str) -> String {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < chars.len() {
            let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
            if rest.starts_with("---") {
                out.push('\u{2014}');
                i += 3;
            } else if rest.starts_with("--") {
                out.push('\u{2013}');
                i += 2;
            } else if rest.starts_with("...") {
                out.push('\u{2026}');
                i += 3;
            } else if rest.starts_with("-?") {
                out.push('\u{AD}');
                i += 2;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        }
        normalise(&out)
    }

    fn normalise(s: &str) -> String {
        s.chars()
            // Quotes become curly quotes, and a quote after a digit becomes a prime.
            .filter(|c| {
                !c.is_whitespace()
                    && !matches!(
                        c,
                        '\u{AD}'
                            | '\''
                            | '"'
                            | '\u{2018}'
                            | '\u{2019}'
                            | '\u{201C}'
                            | '\u{201D}'
                            | '\u{2032}'
                            | '\u{2033}'
                    )
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// Printable ASCII plus a few Latin letters: no bidi or shaping surprises, so the text
        /// can be compared exactly.
        #[test]
        fn escaped_text_survives_at_line_start(s in "[ -~äöåÄÖÅé€]{0,120}") {
            let src = format!("#set page(width: 300pt, height: auto)\n{}", escape_text(&s, Position::LineStart));
            let got = compile_text(&src).map_err(|e| TestCaseError::fail(format!("{e}\nsource: {src:?}")))?;
            prop_assert_eq!(normalise(&got), expected(&s), "source: {:?}", src);
        }

        #[test]
        fn escaped_text_survives_after_an_expression(s in "[ -~äöå]{0,120}") {
            let src = format!(
                "#set page(width: 300pt, height: auto)\n#strong[x]{}",
                escape_text(&s, Position::AfterExpr)
            );
            let got = compile_text(&src).map_err(|e| TestCaseError::fail(format!("{e}\nsource: {src:?}")))?;
            prop_assert_eq!(normalise(&got), expected(&format!("x{s}")), "source: {:?}", src);
        }

        #[test]
        fn escaped_text_survives_inside_a_content_block(s in "[ -~äöå]{0,120}") {
            let src = format!(
                "#set page(width: 300pt, height: auto)\n#emph[{}]",
                escape_text(&s, Position::LineStart)
            );
            let got = compile_text(&src).map_err(|e| TestCaseError::fail(format!("{e}\nsource: {src:?}")))?;
            prop_assert_eq!(normalise(&got), expected(&s), "source: {:?}", src);
        }

        /// Any non-control Unicode: must compile without warnings (text order is not checked
        /// because of bidirectional reordering).
        #[test]
        fn arbitrary_unicode_compiles_without_warnings(s in r"\PC{0,120}") {
            let src = format!("#set page(width: 300pt, height: auto)\n{}", escape_text(&s, Position::LineStart));
            compile_text(&src).map_err(|e| TestCaseError::fail(format!("{e}\nsource: {src:?}")))?;
        }

        /// String literals (URLs, raw code, alt text) round-trip for any text.
        #[test]
        fn string_literals_round_trip(s in r"[ -~\n\täöå]{0,80}") {
            let src = format!("#set page(width: 300pt, height: auto)\n#raw({}, block: true)", typst_string(&s));
            let got = compile_text(&src).map_err(|e| TestCaseError::fail(format!("{e}\nsource: {src:?}")))?;
            prop_assert_eq!(normalise(&got), normalise(&s));
        }
    }
}
