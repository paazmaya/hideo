pub mod emit;
pub mod escape;
pub mod tables;
pub mod lead;

use pulldown_cmark::{Event, Tag, TagEnd, Parser, Options, OffsetIter};
use std::panic;
use crate::page::Page;
use crate::error::{DiagnosticCollector, HideoError, ErrorCode, Warning, WarningCode};
use crate::linkmap::LinkMap;

pub struct MarkdownProcessor {
    pub link_map: LinkMap,
}

impl MarkdownProcessor {
    pub fn new(link_map: LinkMap) -> Self {
        Self { link_map }
    }

    pub fn process(
        &mut self,
        page: &Page,
        diagnostics: &mut DiagnosticCollector,
    ) -> Result<String, Vec<HideoError>> {
        let body = &page.body;
        let opts = markdown_parser_options();

        let result = panic::catch_unwind(|| {
            let parser = Parser::new_ext(body, opts);
            let mut offset_iter = parser.into_offset_iter();
            let mut output = String::new();
            let mut errors = Vec::new();

            while let Some((event, range)) = offset_iter.next() {
                match event {
                    Event::Start(Tag::FootnoteDefinition(_)) => {
                        errors.push(HideoError::new(ErrorCode::E002, "footnotes are not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                        skip_tag(&mut offset_iter, TagEnd::FootnoteDefinition);
                    }
                    Event::Start(Tag::Strikethrough) => {
                        errors.push(HideoError::new(ErrorCode::E002, "strikethrough is not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                        skip_tag(&mut offset_iter, TagEnd::Strikethrough);
                    }
                    Event::Start(Tag::DefinitionList) => {
                        errors.push(HideoError::new(ErrorCode::E002, "definition lists are not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                        skip_tag(&mut offset_iter, TagEnd::DefinitionList);
                    }
                    Event::Start(Tag::Superscript) => {
                        errors.push(HideoError::new(ErrorCode::E002, "superscript is not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                        skip_tag(&mut offset_iter, TagEnd::Superscript);
                    }
                    Event::Start(Tag::Subscript) => {
                        errors.push(HideoError::new(ErrorCode::E002, "subscript is not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                        skip_tag(&mut offset_iter, TagEnd::Subscript);
                    }
                    Event::InlineHtml(html) => {
                        diagnostics.add_warning(Warning::new(WarningCode::W002, "inline HTML is not supported, content dropped")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                    }
                    Event::Html(html) => {
                        errors.push(HideoError::new(ErrorCode::E002, "HTML blocks are not supported")
                            .with_page(page.key.clone())
                            .with_line(page.byte_to_line(range.start)));
                    }
                    _ => {}
                }
            }
            (output, errors)
        });

        match result {
            Ok((_output, errors)) => {
                if !errors.is_empty() {
                    return Err(errors);
                }
                Ok(String::new())
            }
            Err(_) => {
                Err(vec![HideoError::new(ErrorCode::E005, "Markdown parser panic")
                    .with_page(page.key.clone())])
            }
        }
    }
}

fn skip_tag(offset_iter: &mut OffsetIter, end_tag: TagEnd) {
    while let Some((event, _)) = offset_iter.next() {
        if matches!(event, Event::End(tag) if tag == end_tag) {
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