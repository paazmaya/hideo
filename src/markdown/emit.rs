use pulldown_cmark::{Event, Tag, TagEnd, Parser, Options, OffsetIter, CowStr};
use crate::page::Page;
use crate::linkmap::LinkMap;
use crate::error::{DiagnosticCollector, HideoError, ErrorCode};
use crate::markdown::escape::{escape_text, escape_raw_inline, escape_raw_block, Position};
use crate::markdown::tables::{process_table, TableAlignment};
use crate::markdown::lead::{is_lead_paragraph, emit_lead};

pub struct LineMap {
    pub entries: Vec<u32>,
}

pub struct Emitter<'a> {
    page: &'a Page,
    link_map: &'a mut LinkMap,
    diagnostics: &'a mut DiagnosticCollector,
    line_map: LineMap,
    current_line: usize,
    in_paragraph: bool,
    paragraph_events: Vec<Event<'a>>,
}

impl<'a> Emitter<'a> {
    pub fn new(
        page: &'a Page,
        link_map: &'a mut LinkMap,
        diagnostics: &'a mut DiagnosticCollector,
    ) -> Self {
        Self {
            page,
            link_map,
            diagnostics,
            line_map: LineMap { entries: Vec::new() },
            current_line: 0,
            in_paragraph: false,
            paragraph_events: Vec::new(),
        }
    }

    pub fn emit(mut self) -> Result<(String, LineMap), Vec<HideoError>> {
        let mut output = String::new();
        let opts = markdown_parser_options();
        let parser = Parser::new_ext(&self.page.body, opts);
        let mut offset_iter = parser.into_offset_iter();

        while let Some((event, range)) = offset_iter.next() {
            let event_line = self.page.byte_to_line(range.start);
            self.record_line_mapping(event_line);

            match event {
                Event::Start(Tag::Paragraph) => {
                    self.in_paragraph = true;
                    self.paragraph_events.clear();
                }
                Event::End(TagEnd::Paragraph) => {
                    self.in_paragraph = false;
                    self.emit_paragraph(&mut output);
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let mut heading_text = String::new();
                    let mut explicit_id = None;

                    while let Some((e, r)) = offset_iter.next() {
                        self.record_line_mapping(self.page.byte_to_line(r.start));
                        match e {
                            Event::Text(t) | Event::Code(t) => heading_text.push_str(t.as_ref()),
                            Event::Start(Tag::Link { .. }) | Event::Start(Tag::Image { .. }) => {
                                skip_tag(&mut offset_iter);
                            }
                            Event::End(TagEnd::Heading) => break,
                            _ => {}
                        }
                    }

                    for heading in &self.page.headings {
                        if heading.text == heading_text {
                            explicit_id = Some(heading.id.clone());
                            break;
                        }
                    }

                    let label = self.link_map.labels.get(&(self.page.key.clone(), explicit_id.unwrap_or_else(|| generate_slug(&heading_text))))
                        .cloned()
                        .unwrap_or_else(|| format!("<{}>", self.page.key));

                    output.push_str(&format!("#heading(level: {}, outlined: false)[{}] <{}>\n", level, escape_text(&heading_text, Position::Inline), label));
                }
                Event::Start(Tag::BlockQuote) => {
                    output.push_str("#quote[\n");
                }
                Event::End(TagEnd::BlockQuote) => {
                    output.push_str("]\n");
                }
                Event::Start(Tag::CodeBlock(info)) => {
                    let mut code = String::new();
                    while let Some((e, r)) = offset_iter.next() {
                        self.record_line_mapping(self.page.byte_to_line(r.start));
                        if let Event::Text(t) = e {
                            code.push_str(t.as_ref());
                        } else if matches!(e, Event::End(TagEnd::CodeBlock)) {
                            break;
                        }
                    }
                    let lang = info.as_ref().to_string();
                    let valid_lang = if lang.is_empty() || !is_valid_lang_tag(&lang) {
                        None
                    } else {
                        Some(lang)
                    };
                    output.push_str(&escape_raw_block(&code, valid_lang.as_deref()));
                    output.push('\n');
                }
                Event::Start(Tag::List(start)) => {
                    let ordered = start.is_some();
                    output.push_str(&format!("#list({})\n", if ordered { "numbered" } else { "bulleted" }));
                }
                Event::End(TagEnd::List) => {
                    output.push_str("\n");
                }
                Event::Start(Tag::Item) => {
                    output.push_str("  ");
                }
                Event::End(TagEnd::Item) => {
                    output.push('\n');
                }
                Event::Start(Tag::Table(alignments)) => {
                    let mut header_cells = Vec::new();
                    let mut body_rows = Vec::new();
                    let mut in_header = true;
                    let mut current_row = Vec::new();
                    let mut current_cell = Vec::new();

                    while let Some((e, r)) = offset_iter.next() {
                        self.record_line_mapping(self.page.byte_to_line(r.start));
                        match e {
                            Event::Start(Tag::TableHead) => {
                                in_header = true;
                            }
                            Event::End(TagEnd::TableHead) => {
                                in_header = false;
                            }
                            Event::Start(Tag::TableRow) => {
                                current_row.clear();
                            }
                            Event::End(TagEnd::TableRow) => {
                                if in_header {
                                    header_cells = current_row.clone();
                                } else {
                                    body_rows.push(current_row.clone());
                                }
                            }
                            Event::Start(Tag::TableCell) => {
                                current_cell.clear();
                            }
                            Event::End(TagEnd::TableCell) => {
                                current_row.push(current_cell.clone());
                            }
                            Event::Text(t) | Event::Code(t) => {
                                current_cell.push(Event::Text(t.clone()));
                            }
                            Event::SoftBreak | Event::HardBreak => {
                                current_cell.push(Event::SoftBreak);
                            }
                            Event::End(TagEnd::Table) => break,
                            _ => {}
                        }
                    }

                    let table_output = process_table(
                        &alignments,
                        &header_cells,
                        &body_rows,
                        450.0,
                        10.5,
                        self.diagnostics,
                        &self.page.key,
                        self.current_line,
                    );
                    output.push_str(&table_output);
                }
                Event::Start(Tag::Emphasis) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::Start(Tag::Emphasis));
                    } else {
                        output.push('*');
                    }
                }
                Event::End(TagEnd::Emphasis) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::End(TagEnd::Emphasis));
                    } else {
                        output.push('*');
                    }
                }
                Event::Start(Tag::Strong) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::Start(Tag::Strong));
                    } else {
                        output.push_str("**");
                    }
                }
                Event::End(TagEnd::Strong) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::End(TagEnd::Strong));
                    } else {
                        output.push_str("**");
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    let resolved = self.link_map.resolve_link(self.page, &dest_url, self.current_line);
                    if let Some(link) = resolved {
                        if self.in_paragraph {
                            self.paragraph_events.push(Event::Text(link.into()));
                        } else {
                            output.push_str(&link);
                        }
                    } else {
                        if self.in_paragraph {
                            self.paragraph_events.push(Event::Text(format!("[{}]", dest_url).into()));
                        } else {
                            output.push_str(&format!("[{}]", dest_url));
                        }
                    }
                }
                Event::End(TagEnd::Link) => {}
                Event::Text(text) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::Text(text.clone()));
                    } else {
                        output.push_str(&escape_text(text.as_ref(), Position::Inline));
                    }
                }
                Event::Code(text) => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::Code(text.clone()));
                    } else {
                        output.push_str(&escape_raw_inline(text.as_ref()));
                    }
                }
                Event::SoftBreak => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::SoftBreak);
                    } else {
                        output.push(' ');
                    }
                }
                Event::HardBreak => {
                    if self.in_paragraph {
                        self.paragraph_events.push(Event::HardBreak);
                    } else {
                        output.push_str("\\\n");
                    }
                }
                Event::Rule => {
                    output.push_str("#rule(width: 30%, thickness: 0.6pt, stroke: rgb(\"D3CCE4\"))\n");
                }
                _ => {}
            }
        }

        Ok((output, self.line_map))
    }

    fn record_line_mapping(&mut self, markdown_line: usize) {
        let typst_line = self.line_map.entries.len() as u32 + 1;
        if self.line_map.entries.len() < typst_line as usize {
            self.line_map.entries.resize(typst_line as usize, 0);
        }
        self.line_map.entries[typst_line as usize - 1] = markdown_line as u32;
        self.current_line = markdown_line;
    }

    fn emit_paragraph(&mut self, output: &mut String) {
        if self.paragraph_events.is_empty() {
            return;
        }

        if is_lead_paragraph(&self.paragraph_events) {
            let mut strong_text = String::new();
            let mut rest = Vec::new();
            let mut in_strong = false;
            let mut strong_done = false;

            for event in &self.paragraph_events {
                match event {
                    Event::Start(Tag::Strong) => in_strong = true,
                    Event::End(TagEnd::Strong) => {
                        in_strong = false;
                        strong_done = true;
                    }
                    Event::Text(t) | Event::Code(t) => {
                        if in_strong {
                            strong_text.push_str(t.as_ref());
                        } else if strong_done {
                            rest.push(event.clone());
                        }
                    }
                    _ => {
                        if strong_done {
                            rest.push(event.clone());
                        }
                    }
                }
            }

            output.push_str(&emit_lead(&strong_text, &rest));
            return;
        }

        for event in &self.paragraph_events {
            output.push_str(&self.emit_inline_event(event));
        }
        output.push('\n');
    }

    fn emit_inline_event(&self, event: &Event) -> String {
        match event {
            Event::Text(t) => escape_text(t.as_ref(), Position::Inline),
            Event::Code(t) => format!("`{}`", escape_text(t.as_ref(), Position::Inline)),
            Event::SoftBreak => " ".to_string(),
            Event::HardBreak => "\\\n".to_string(),
            Event::Start(Tag::Emphasis) => "*".to_string(),
            Event::End(TagEnd::Emphasis) => "*".to_string(),
            Event::Start(Tag::Strong) => "**".to_string(),
            Event::End(TagEnd::Strong) => "**".to_string(),
            Event::Start(Tag::Link { dest_url, .. }) => {
                format!("#link(\"{}\")[", escape_typst_string(dest_url.as_ref()))
            }
            Event::End(TagEnd::Link) => "]".to_string(),
            _ => String::new(),
        }
    }
}

fn skip_tag(offset_iter: &mut OffsetIter) {
    while let Some((event, _)) = offset_iter.next() {
        if matches!(event, Event::End(TagEnd::Link) | Event::End(TagEnd::Image)) {
            break;
        }
    }
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

fn is_valid_lang_tag(tag: &str) -> bool {
    !tag.is_empty() && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '#' || c == '-' || c == '.')
}

fn escape_typst_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}