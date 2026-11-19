//! A parsed page: key, headings with unique ids, line index (plan F5, F6, 5, 7.2 steps 3-4).

use std::collections::HashSet;
use std::panic::{self, AssertUnwindSafe};

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

use crate::discover::SourcePage;
use crate::error::{ErrorCode, HideoError};
use crate::frontmatter::Frontmatter;
use crate::markdown::parser_options;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// Final id, unique within the page.
    pub id: String,
    pub explicit: bool,
    /// File line of the heading.
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Page {
    /// Typst label of the page itself, unique across the tier (F6).
    pub key: String,
    /// Path relative to the source folder, forward slashes. Used in diagnostics and link
    /// resolution.
    pub rel_path: String,
    pub tier: String,
    pub frontmatter: Frontmatter,
    pub body: String,
    pub headings: Vec<Heading>,
    line_starts: Vec<usize>,
}

impl Page {
    /// Build a page. `key` is supplied by the caller because uniqueness is a tier property.
    pub fn build(source: SourcePage, key: String) -> Result<Page, Vec<HideoError>> {
        let line_starts = build_line_starts(&source.body);
        let mut page = Page {
            key,
            rel_path: source.rel_path,
            tier: source.tier,
            frontmatter: source.frontmatter,
            body: source.body,
            headings: Vec::new(),
            line_starts,
        };
        page.headings = page.extract_headings()?;
        Ok(page)
    }

    /// File line (1-based) of a byte offset in the body.
    pub fn byte_to_line(&self, byte_offset: usize) -> usize {
        let index = match self.line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i - 1, // line_starts[0] == 0 so i >= 1
        };
        self.frontmatter.body_line_offset + index + 1
    }

    pub fn title(&self) -> &str {
        self.frontmatter.title.as_deref().unwrap_or("")
    }

    fn extract_headings(&self) -> Result<Vec<Heading>, Vec<HideoError>> {
        // Guard against pulldown-cmark panics on hostile input (plan 5.1).
        let found = panic::catch_unwind(AssertUnwindSafe(|| self.scan_headings()));
        let raw = found.map_err(|_| {
            vec![
                HideoError::new(ErrorCode::E005, "the Markdown parser panicked on this page")
                    .with_page(self.rel_path.clone()),
            ]
        })?;

        let mut errors = Vec::new();
        let explicit_ids: HashSet<&str> =
            raw.iter().filter_map(|h| h.explicit.as_deref()).collect();
        let mut used: HashSet<String> = HashSet::new();
        let mut headings = Vec::with_capacity(raw.len());

        for h in &raw {
            let (id, explicit) = match &h.explicit {
                Some(id) => {
                    if !is_valid_explicit_id(id) {
                        errors.push(
                            HideoError::new(
                                ErrorCode::E003,
                                format!("heading id `{id}` must match [a-zA-Z][a-zA-Z0-9_-]*"),
                            )
                            .with_location(self.rel_path.clone(), h.line),
                        );
                    } else if used.contains(id) {
                        errors.push(
                            HideoError::new(
                                ErrorCode::E003,
                                format!("heading id `{id}` is used twice on this page"),
                            )
                            .with_location(self.rel_path.clone(), h.line),
                        );
                    }
                    (id.clone(), true)
                }
                None => {
                    let base = slugify(&h.text);
                    let mut candidate = base.clone();
                    let mut n = 0;
                    // `-1`, `-2`, ... in document order; also avoid explicit ids (F5).
                    while used.contains(&candidate) || explicit_ids.contains(candidate.as_str()) {
                        n += 1;
                        candidate = format!("{base}-{n}");
                    }
                    (candidate, false)
                }
            };
            used.insert(id.clone());
            headings.push(Heading {
                level: h.level,
                text: h.text.clone(),
                id,
                explicit,
                line: h.line,
            });
        }

        if errors.is_empty() {
            Ok(headings)
        } else {
            Err(errors)
        }
    }

    fn scan_headings(&self) -> Vec<RawHeading> {
        let parser = Parser::new_ext(&self.body, parser_options()).into_offset_iter();
        let mut out = Vec::new();
        let mut current: Option<RawHeading> = None;
        for (event, range) in parser {
            match event {
                Event::Start(Tag::Heading { level, id, .. }) => {
                    current = Some(RawHeading {
                        level: level as u8,
                        text: String::new(),
                        explicit: id.map(|s| s.to_string()),
                        line: self.byte_to_line(range.start),
                    });
                }
                Event::End(TagEnd::Heading(_)) => out.extend(current.take()),
                Event::Text(t) | Event::Code(t) => {
                    if let Some(h) = current.as_mut() {
                        h.text.push_str(&t);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(h) = current.as_mut() {
                        h.text.push(' ');
                    }
                }
                _ => {}
            }
        }
        out
    }
}

struct RawHeading {
    level: u8,
    text: String,
    explicit: Option<String>,
    line: usize,
}

fn build_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
    starts
}

/// Heading slug (F5): lowercase, keep alphanumerics, `_` and `-`, each whitespace character
/// becomes a hyphen (runs do not collapse), other punctuation is dropped, empty becomes
/// `section`.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if c == '_' || c == '-' {
            slug.push(c);
        } else if c.is_whitespace() {
            slug.push('-');
        }
    }
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

/// `^[a-zA-Z][a-zA-Z0-9_-]*$`
pub fn is_valid_explicit_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Label-safe key for a page path (`required/01_intro.md` -> `required-01-intro`).
pub fn page_key(rel_path: &str) -> String {
    let without_ext = rel_path.strip_suffix(".md").unwrap_or(rel_path);
    let mut key = String::new();
    let mut pending_hyphen = false;
    for c in without_ext.chars() {
        if c.is_alphanumeric() {
            if pending_hyphen && !key.is_empty() {
                key.push('-');
            }
            pending_hyphen = false;
            key.extend(c.to_lowercase());
        } else {
            pending_hyphen = true;
        }
    }
    if key.is_empty() {
        "page".to_string()
    } else {
        key
    }
}

/// Assign unique keys to the pages of one tier, preserving order (F6). Collisions such as
/// `a_b.md` and `a-b.md` get `-2`, `-3`, ... suffixes.
pub fn assign_keys(sources: &[SourcePage]) -> Vec<String> {
    let mut used: HashSet<String> = HashSet::new();
    sources
        .iter()
        .map(|s| {
            let base = page_key(&s.rel_path);
            let mut key = base.clone();
            let mut n = 1;
            while !used.insert(key.clone()) {
                n += 1;
                key = format!("{base}-{n}");
            }
            key
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    fn source(body: &str) -> SourcePage {
        SourcePage {
            tier: "t".into(),
            rel_path: "t/p.md".into(),
            abs_path: Utf8PathBuf::from("t/p.md"),
            frontmatter: Frontmatter {
                body_line_offset: 4,
                ..Default::default()
            },
            body: body.to_string(),
        }
    }

    fn page(body: &str) -> Page {
        Page::build(source(body), "t-p".into()).unwrap()
    }

    fn ids(p: &Page) -> Vec<&str> {
        p.headings.iter().map(|h| h.id.as_str()).collect()
    }

    #[test]
    fn test_heading_id_slug_unicode() {
        let p = page("# Älä ÖÅ tee\n\n## Kolme sanaa\n");
        assert_eq!(ids(&p), ["älä-öå-tee", "kolme-sanaa"]);
    }

    #[test]
    fn test_heading_id_explicit() {
        let p = page("## Setup {#install-guide}\n");
        assert_eq!(ids(&p), ["install-guide"]);
        assert!(p.headings[0].explicit);
        assert_eq!(p.headings[0].text, "Setup");
    }

    #[test]
    fn test_heading_id_explicit_invalid_errors() {
        for bad in ["1abc", "a b", "ä"] {
            let result = Page::build(source(&format!("## T {{#{bad}}}\n")), "k".into());
            // `a b` is not parsed as an id by pulldown-cmark, the others are invalid.
            if let Err(errors) = result {
                assert_eq!(errors[0].code, ErrorCode::E003, "{bad}");
                assert_eq!(errors[0].line, Some(5));
                assert_eq!(errors[0].page.as_deref(), Some("t/p.md"));
            } else {
                assert_eq!(bad, "a b");
            }
        }
    }

    #[test]
    fn test_heading_id_duplicate_suffix() {
        let p = page("## Setup\n\n## Setup\n\n## Setup\n");
        assert_eq!(ids(&p), ["setup", "setup-1", "setup-2"]);
    }

    #[test]
    fn test_heading_id_duplicate_explicit_errors() {
        let e = Page::build(source("## A {#x}\n\n## B {#x}\n"), "k".into()).unwrap_err();
        assert_eq!(e[0].code, ErrorCode::E003);
    }

    #[test]
    fn test_heading_id_generated_avoids_explicit() {
        let p = page("## Setup {#setup}\n\n## Setup\n");
        assert_eq!(ids(&p), ["setup", "setup-1"]);
    }

    #[test]
    fn test_heading_id_empty_fallback_section() {
        let p = page("## !!!\n\n## ???\n");
        assert_eq!(ids(&p), ["section", "section-1"]);
    }

    #[test]
    fn test_heading_id_double_space_two_hyphens() {
        assert_eq!(slugify("a  b"), "a--b");
        assert_eq!(slugify("A, b: c!"), "a-b-c");
        assert_eq!(slugify("snake_case-and-more"), "snake_case-and-more");
    }

    #[test]
    fn test_heading_inside_code_ignored() {
        let p = page("```\n# not a heading\n```\n\n    # nor this\n\n## Real\n");
        assert_eq!(ids(&p), ["real"]);
    }

    #[test]
    fn test_heading_text_includes_inline_code_and_link_text() {
        let p = page("## Use `cargo` and [the docs](x.md)\n");
        assert_eq!(p.headings[0].text, "Use cargo and the docs");
        assert_eq!(p.headings[0].id, "use-cargo-and-the-docs");
    }

    #[test]
    fn test_body_line_numbers_include_frontmatter_offset() {
        let p = page("para\n\n## Third\n");
        // frontmatter occupies 4 lines; body line index 2 is file line 7.
        assert_eq!(p.headings[0].line, 7);
        assert_eq!(p.byte_to_line(0), 5);
    }

    #[test]
    fn test_page_key_and_uniqueness() {
        assert_eq!(page_key("required/01_intro.md"), "required-01-intro");
        assert_eq!(page_key("required/Mixed Case.md"), "required-mixed-case");
        assert_eq!(
            page_key("t/nested/page with spaces.md"),
            "t-nested-page-with-spaces"
        );
        let mk = |p: &str| SourcePage {
            rel_path: p.into(),
            ..source("")
        };
        let keys = assign_keys(&[mk("t/a_b.md"), mk("t/a-b.md"), mk("t/a b.md")]);
        assert_eq!(keys, ["t-a-b", "t-a-b-2", "t-a-b-3"]);
    }

    #[test]
    fn test_heading_breaks() {
        let p = page("Hello\nworld\n=====\n");
        assert_eq!(p.headings[0].text, "Hello world");
    }

    #[test]
    fn test_page_key_empty_fallback() {
        assert_eq!(page_key("---.md"), "page");
    }

    #[test]
    fn test_same_heading_text_on_two_pages_gets_distinct_labels() {
        let a = page("## Setup\n");
        let mut b = page("## Setup\n");
        b.key = "t-q".into();
        assert_eq!(a.headings[0].id, b.headings[0].id);
        assert_ne!(
            format!("{}--{}", a.key, a.headings[0].id),
            format!("{}--{}", b.key, b.headings[0].id)
        );
    }

    // ---- property tests (plan 8.3) ------------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        /// Random heading lists (with duplicates, punctuation and explicit ids): all ids on a
        /// page are unique.
        #[test]
        fn heading_ids_are_unique(
            headings in prop::collection::vec(("[A-Za-z!? ]{0,6}", prop::option::of("[a-z]{1,3}")), 0..25)
        ) {
            let mut body = String::new();
            for (text, explicit) in &headings {
                match explicit {
                    Some(id) => body.push_str(&format!("## {text} x {{#{id}}}\n\n")),
                    None => body.push_str(&format!("## {text} x\n\n")),
                }
            }
            // Duplicate explicit ids are an error by design; only unique-explicit inputs build.
            if let Ok(p) = Page::build(source(&body), "k".into()) {
                let ids: Vec<&str> = p.headings.iter().map(|h| h.id.as_str()).collect();
                let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
                prop_assert_eq!(unique.len(), ids.len(), "{:?}", ids);
                prop_assert_eq!(ids.len(), headings.len());
            }
        }

        /// The byte-to-line index agrees with a naive newline count.
        #[test]
        fn line_index_agrees_with_naive_counting(
            lines in prop::collection::vec("[a-zä ]{0,12}", 0..40),
        ) {
            let body = lines.join("\n");
            let p = Page::build(source(&body), "k".into()).unwrap();
            for offset in 0..=body.len() {
                if !body.is_char_boundary(offset) {
                    continue;
                }
                let naive = body[..offset].matches('\n').count() + 1 + p.frontmatter.body_line_offset;
                prop_assert_eq!(p.byte_to_line(offset), naive, "offset {}", offset);
            }
        }
    }
}
