use std::collections::HashMap;
use camino::Utf8PathBuf;
use pulldown_cmark::{Event, Tag, TagEnd, Parser, Options, OffsetIter};
use crate::frontmatter::Frontmatter;
use crate::error::{HideoError, ErrorCode, DiagnosticCollector};

#[derive(Debug, Clone)]
pub struct Heading {
    pub level: usize,
    pub text: String,
    pub id: String,
    pub explicit_id: bool,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub key: String,
    pub path: Utf8PathBuf,
    pub tier: String,
    pub frontmatter: Frontmatter,
    pub body: String,
    pub headings: Vec<Heading>,
    pub line_starts: Vec<usize>,
}

impl Page {
    pub fn new(
        key: String,
        path: Utf8PathBuf,
        tier: String,
        frontmatter: Frontmatter,
        body: String,
    ) -> Self {
        let line_starts = build_line_starts(&body);
        let mut page = Self {
            key,
            path,
            tier,
            frontmatter,
            body,
            headings: Vec::new(),
            line_starts,
        };
        page.extract_headings();
        page
    }

    fn extract_headings(&mut self) {
        let opts = markdown_parser_options();
        let parser = Parser::new_ext(&self.body, opts);
        let mut offset_iter = parser.into_offset_iter();

        let mut heading_counts: HashMap<String, usize> = HashMap::new();

        while let Some((event, range)) = offset_iter.next() {
            if let Event::Start(Tag::Heading { level, .. }) = event {
                let mut heading_text = String::new();
                let mut explicit_id = None;
                let start_line = self.byte_to_line(range.start);

                while let Some((event, _)) = offset_iter.next() {
                    match event {
                        Event::Text(text) | Event::Code(text) => {
                            heading_text.push_str(&text);
                        }
                        Event::Start(Tag::Link { .. }) | Event::Start(Tag::Image { .. }) => {
                            while let Some((e, _)) = offset_iter.next() {
                                if matches!(e, Event::End(TagEnd::Link) | Event::End(TagEnd::Image)) {
                                    break;
                                }
                            }
                        }
                        Event::End(TagEnd::Heading) => break,
                        _ => {}
                    }
                }

                let slug = if let Some(id) = explicit_id {
                    explicit_id = Some(id.clone());
                    true
                } else {
                    false
                };

                let id = if let Some(id) = explicit_id {
                    if !is_valid_explicit_id(&id) {
                        // Error will be reported elsewhere
                        id
                    } else {
                        id
                    }
                } else {
                    generate_slug(&heading_text)
                };

                let count = heading_counts.entry(id.clone()).or_insert(0);
                let final_id = if *count == 0 {
                    id.clone()
                } else {
                    *count += 1;
                    format!("{}-{}", id, count)
                };
                if *count == 0 {
                    *count = 1;
                }

                self.headings.push(Heading {
                    level: level as usize,
                    text: heading_text,
                    id: final_id,
                    explicit_id: slug,
                });
            }
        }
    }

    pub fn byte_to_line(&self, byte_offset: usize) -> usize {
        let idx = self.line_starts.binary_search(&byte_offset).unwrap_or_else(|i| i.saturating_sub(1));
        self.frontmatter.body_line_offset + idx
    }
}

fn build_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, c) in text.char_indices() {
        if c == '\n' {
            starts.push(i + 1);
        }
    }
    starts
}

fn markdown_parser_options() -> Options {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_DEFINITION_LIST);
    opts.insert(Options::ENABLE_SUPERSCRIPT);
    opts.insert(Options::ENABLE_SUBSCRIPT);
    opts
}

fn generate_slug(text: &str) -> String {
    let mut slug = String::new();
    let mut prev_was_hyphen = false;

    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            slug.push(c.to_ascii_lowercase());
            prev_was_hyphen = false;
        } else if c.is_whitespace() {
            if !prev_was_hyphen && !slug.is_empty() {
                slug.push('-');
                prev_was_hyphen = true;
            }
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn is_valid_explicit_id(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let mut chars = id.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}