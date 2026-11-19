//! Markdown events to Typst markup (plan 7.5, F9).
//!
//! The output is a list of [`Line`]s, each remembering the Markdown line it came from, so the
//! line map for diagnostics (7.9) falls out for free. Inline content is rendered with Typst
//! function calls (`#strong[..]`, `#emph[..]`, `#link(..)[..]`) rather than `*`/`_` markup,
//! because Typst does not honour intra-word delimiters the way Markdown does.

use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};

use pulldown_cmark::{CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};

use crate::error::{DiagnosticCollector, ErrorCode, HideoError, Warning, WarningCode};
use crate::linkmap::{LinkMap, Resolved};
use crate::markdown::escape::{Position, escape_text, is_valid_lang_tag, typst_string};
use crate::markdown::lead::detect_lead;
use crate::markdown::parser_options;
use crate::markdown::tables::{self, Cell, TableData, TableRow};
use crate::page::Page;

/// One line of generated Typst and the Markdown file line that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub md_line: usize,
}

impl Line {
    fn new(text: impl Into<String>, md_line: usize) -> Self {
        Self {
            text: text.into(),
            md_line,
        }
    }
}

/// Resolves things the emitter cannot produce itself: body images and diagrams.
pub trait AssetSink {
    /// A Markdown body image. Returns the virtual path to use in Typst, or `None` if the image
    /// is unavailable (the sink records the warning).
    fn body_image(&mut self, page: &Page, dest: &str, line: usize) -> Option<String>;
    /// A Mermaid diagram. Returns the virtual path of the SVG the diagram will be rendered to,
    /// or `None` when diagrams are switched off (the source is then shown as a code block).
    fn diagram(&mut self, page: &Page, source: &str, line: usize) -> Option<String>;
}

/// Sink for tests: images are always found, diagrams get a fixed name.
#[derive(Debug, Default)]
pub struct NoAssets;

impl AssetSink for NoAssets {
    fn body_image(&mut self, _page: &Page, dest: &str, _line: usize) -> Option<String> {
        Some(format!("images/{dest}"))
    }
    fn diagram(&mut self, _page: &Page, _source: &str, _line: usize) -> Option<String> {
        Some("diagrams/test.svg".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct PageMarkup {
    /// Generated Typst for the page body.
    pub text: String,
    /// Entry `i` is the Markdown file line of generated line `i` (0-based).
    pub line_map: Vec<u32>,
}

#[derive(Debug)]
pub struct Emitted {
    pub markup: PageMarkup,
    pub diag: DiagnosticCollector,
}

/// Emit one page. Never panics: a parser panic becomes an E005 error naming the page.
pub fn emit_page(
    page: &Page,
    links: &mut LinkMap,
    assets: &mut dyn AssetSink,
    text_area_pt: (f64, f64),
) -> Emitted {
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut emitter = Emitter::new(page, links, assets, text_area_pt);
        let lines = emitter.blocks(None);
        (lines, emitter.diag)
    }));
    match result {
        Ok((lines, diag)) => Emitted {
            markup: PageMarkup {
                text: lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                line_map: lines.iter().map(|l| l.md_line as u32).collect(),
            },
            diag,
        },
        Err(_) => {
            let mut diag = DiagnosticCollector::default();
            diag.add_error(
                HideoError::new(ErrorCode::E005, "the Markdown parser panicked on this page")
                    .with_page(page.rel_path.clone()),
            );
            Emitted {
                markup: PageMarkup {
                    text: String::new(),
                    line_map: Vec::new(),
                },
                diag,
            }
        }
    }
}

struct Emitter<'a, 'l> {
    page: &'a Page,
    events: Vec<(Event<'a>, Range<usize>)>,
    pos: usize,
    links: &'l mut LinkMap,
    assets: &'l mut dyn AssetSink,
    diag: DiagnosticCollector,
    heading_index: usize,
    last_heading_level: Option<u8>,
    /// Text area (width, height) in points.
    text_area_pt: (f64, f64),
    /// Inside a table cell: break long tokens and collect plain text for measuring.
    in_table: bool,
    plain: String,
}

fn is_block_start(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Paragraph
            | Tag::Heading { .. }
            | Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::MetadataBlock(_)
    )
}

impl<'a, 'l> Emitter<'a, 'l> {
    fn new(
        page: &'a Page,
        links: &'l mut LinkMap,
        assets: &'l mut dyn AssetSink,
        text_area_pt: (f64, f64),
    ) -> Self {
        let events = Parser::new_ext(&page.body, parser_options())
            .into_offset_iter()
            .collect();
        Self {
            page,
            events,
            pos: 0,
            links,
            assets,
            diag: DiagnosticCollector::default(),
            heading_index: 0,
            last_heading_level: None,
            text_area_pt,
            in_table: false,
            plain: String::new(),
        }
    }

    // ---- diagnostics --------------------------------------------------------------------

    fn line_at(&self, index: usize) -> usize {
        match self.events.get(index) {
            Some((_, range)) => self.page.byte_to_line(range.start),
            None => self.page.byte_to_line(self.page.body.len()),
        }
    }

    fn error(&mut self, code: ErrorCode, message: impl Into<String>, line: usize) {
        self.diag.add_error(
            HideoError::new(code, message).with_location(self.page.rel_path.clone(), line),
        );
    }

    fn warn(&mut self, code: WarningCode, message: impl Into<String>, line: usize) {
        self.diag.add_warning(
            Warning::new(code, message).with_location(self.page.rel_path.clone(), line),
        );
    }

    fn unsupported(&mut self, what: &str, line: usize) {
        self.error(ErrorCode::E002, format!("{what} are not supported"), line);
    }

    // ---- navigation ---------------------------------------------------------------------

    /// Skip a container whose `Start` was just consumed, through its matching `End`.
    fn skip_container(&mut self) {
        let mut depth = 1;
        while let Some((event, _)) = self.events.get(self.pos) {
            self.pos += 1;
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    }

    /// Text and code inside the container whose `Start` was just consumed, consuming it.
    fn collect_plain_text(&mut self) -> String {
        let mut text = String::new();
        let mut depth = 1;
        while let Some((event, _)) = self.events.get(self.pos) {
            self.pos += 1;
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Event::Text(t) | Event::Code(t) => text.push_str(t),
                Event::SoftBreak | Event::HardBreak => text.push(' '),
                _ => {}
            }
        }
        text
    }

    /// Plain text of the container starting at `self.pos`, without consuming it.
    fn peek_plain_text(&self) -> String {
        let mut text = String::new();
        let mut depth = 1;
        for (event, _) in &self.events[self.pos..] {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Event::Text(t) | Event::Code(t) => text.push_str(t),
                _ => {}
            }
        }
        text
    }

    // ---- blocks -------------------------------------------------------------------------

    /// Emit blocks until `end` (consumed) or the end of the events when `end` is `None`.
    fn blocks(&mut self, end: Option<TagEnd>) -> Vec<Line> {
        let mut out: Vec<Line> = Vec::new();
        while let Some((event, _)) = self.events.get(self.pos).cloned() {
            let line = self.line_at(self.pos);
            match event {
                Event::End(e) if Some(e) == end => {
                    self.pos += 1;
                    break;
                }
                Event::End(_) => self.pos += 1,
                Event::Start(tag) => {
                    self.pos += 1;
                    let block = self.block(tag, line);
                    push_block(&mut out, block);
                }
                Event::Rule => {
                    self.pos += 1;
                    push_block(&mut out, vec![Line::new("#hr()", line)]);
                }
                Event::Html(_) | Event::InlineHtml(_) if end.is_none() => {
                    // Stray HTML outside an HtmlBlock cannot happen; treat like inline.
                    self.pos += 1;
                }
                _ => {
                    // Inline content at block level: tight list items have no paragraph.
                    let before = self.pos;
                    let text = self.inline(None, Position::LineStart);
                    self.pos = self.pos.max(before + 1);
                    if !text.is_empty() {
                        push_block(&mut out, vec![Line::new(text, line)]);
                    }
                }
            }
        }
        out
    }

    fn block(&mut self, tag: Tag<'a>, line: usize) -> Vec<Line> {
        match tag {
            Tag::Paragraph => self.paragraph(line),
            Tag::Heading { level, .. } => self.heading(level as u8, line),
            Tag::BlockQuote(kind) => self.blockquote(kind, line),
            Tag::CodeBlock(kind) => self.code_block(kind, line),
            Tag::List(start) => self.list(start, line),
            Tag::Table(aligns) => self.table(aligns, line),
            Tag::HtmlBlock => self.html_block(line),
            Tag::FootnoteDefinition(_) => {
                self.unsupported("footnotes", line);
                self.skip_container();
                Vec::new()
            }
            Tag::DefinitionList => {
                self.unsupported("definition lists", line);
                self.skip_container();
                Vec::new()
            }
            Tag::MetadataBlock(_) => {
                self.error(ErrorCode::E002, "metadata blocks are not supported", line);
                self.skip_container();
                Vec::new()
            }
            // Inline tags at block level (tight list items): rewind and render as inline.
            _ => {
                self.pos -= 1;
                let text = self.inline(None, Position::LineStart);
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![Line::new(text, line)]
                }
            }
        }
    }

    fn blockquote(
        &mut self,
        kind: Option<pulldown_cmark::BlockQuoteKind>,
        line: usize,
    ) -> Vec<Line> {
        let inner = self.blocks(Some(TagEnd::BlockQuote(kind)));
        let mut lines = vec![Line::new("#quote(block: true)[", line)];
        lines.extend(inner);
        lines.push(Line::new("]", line));
        lines
    }

    fn paragraph(&mut self, line: usize) -> Vec<Line> {
        if let Some(lines) = self.block_image(line) {
            return lines;
        }
        let window: Vec<Event> = self.events[self.pos..]
            .iter()
            .take(3)
            .map(|(e, _)| e.clone())
            .collect();
        let (lead, start) = if let Some((text, consumed)) = detect_lead(&window) {
            let text = text.to_string();
            self.pos += consumed;
            (Some(text), Position::AfterExpr)
        } else {
            (None, Position::LineStart)
        };
        let rest = self.inline(Some(TagEnd::Paragraph), start);
        let text = if let Some(label) = lead {
            format!(
                "#lead[{}]{rest}",
                escape_text(label.trim_end(), Position::LineStart)
            )
        } else {
            rest
        };
        vec![Line::new(text, line)]
    }

    /// A paragraph consisting of exactly one image becomes a block image.
    fn block_image(&mut self, line: usize) -> Option<Vec<Line>> {
        let Some((Event::Start(Tag::Image { dest_url, .. }), _)) = self.events.get(self.pos) else {
            return None;
        };
        let dest = dest_url.to_string();
        // Find the matching End(Image) and require End(Paragraph) right after it.
        let mut depth = 0;
        let mut end_index = None;
        for (i, (event, _)) in self.events.iter().enumerate().skip(self.pos) {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        end_index = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end_index = end_index?;
        if !matches!(
            self.events.get(end_index + 1),
            Some((Event::End(TagEnd::Paragraph), _))
        ) {
            return None;
        }
        self.pos += 1;
        let alt = self.collect_plain_text();
        self.pos += 1; // End(Paragraph)
        Some(vec![Line::new(self.image_markup(&dest, &alt, line), line)])
    }

    fn image_markup(&mut self, dest: &str, alt: &str, line: usize) -> String {
        if let Some(path) = self.assets.body_image(self.page, dest, line) {
            let alt = alt.trim();
            if alt.is_empty() {
                format!("#hideo-image({}, alt: none)", typst_string(&path))
            } else {
                format!(
                    "#hideo-image({}, alt: {})",
                    typst_string(&path),
                    typst_string(alt)
                )
            }
        } else {
            escape_text(alt, Position::LineStart)
        }
    }

    fn heading(&mut self, level: u8, line: usize) -> Vec<Line> {
        let end = TagEnd::Heading(pulldown_level(level));
        let inner = self.inline(Some(end), Position::LineStart);
        let index = self.heading_index;
        self.heading_index += 1;
        if let Some(prev) = self.last_heading_level
            && level > prev + 1
        {
            self.warn(
                WarningCode::W007,
                format!("heading level jumps from H{prev} to H{level}; screen reader navigation relies on consecutive levels"),
                line,
            );
        }
        self.last_heading_level = Some(level);
        let label = match self.page.headings.get(index) {
            Some(h) => LinkMap::heading_label(&self.page.key, &h.id),
            None => format!("{}--h{index}", self.page.key),
        };
        vec![Line::new(
            format!("#heading(level: {level})[{inner}] <{label}>"),
            line,
        )]
    }

    fn code_block(&mut self, kind: CodeBlockKind<'a>, line: usize) -> Vec<Line> {
        let mut code = String::new();
        while let Some((event, _)) = self.events.get(self.pos) {
            self.pos += 1;
            match event {
                Event::End(TagEnd::CodeBlock) => break,
                Event::Text(t) => code.push_str(t),
                _ => {}
            }
        }
        if code.ends_with('\n') {
            code.pop();
        }
        let info = match &kind {
            CodeBlockKind::Fenced(info) => info.to_string(),
            CodeBlockKind::Indented => String::new(),
        };
        let (lang, rest) = match info.trim().split_once(char::is_whitespace) {
            Some((lang, rest)) => (lang, rest.trim()),
            None => (info.trim(), ""),
        };

        if lang == "mermaid" {
            return self.diagram(&code, rest, line);
        }

        // Raw text goes into a string literal: nothing in it can be read as markup, and no
        // delimiter can be forged by the content (plan 7.5 "design B", used for code).
        let lang_arg = if lang.is_empty() {
            String::new()
        } else if is_valid_lang_tag(lang) {
            format!(", lang: {}", typst_string(lang))
        } else {
            self.warn(
                WarningCode::W010,
                format!("code block language `{lang}` is not a valid tag and was dropped"),
                line,
            );
            String::new()
        };
        vec![Line::new(
            format!("#raw({}, block: true{lang_arg})", typst_string(&code)),
            line,
        )]
    }

    fn diagram(&mut self, source: &str, info_rest: &str, line: usize) -> Vec<Line> {
        let Some(path) = self.assets.diagram(self.page, source, line) else {
            let raw = format!(
                "#raw({}, block: true, lang: \"mermaid\")",
                typst_string(source)
            );
            return vec![Line::new(raw, line)];
        };
        let alt = diagram_alt(info_rest, source);
        let alt = match alt {
            Some(a) => a,
            None => {
                self.warn(
                    WarningCode::W008,
                    "diagram has no alt text; add one after the fence language: ```mermaid \"Alt text\" or an accTitle line",
                    line,
                );
                "Diagram".to_string()
            }
        };
        vec![Line::new(
            format!(
                "#hideo-diagram({}, alt: {})",
                typst_string(&path),
                typst_string(&alt)
            ),
            line,
        )]
    }

    fn html_block(&mut self, line: usize) -> Vec<Line> {
        let mut html = String::new();
        while let Some((event, _)) = self.events.get(self.pos) {
            self.pos += 1;
            match event {
                Event::End(TagEnd::HtmlBlock) => break,
                Event::Html(t) => html.push_str(t),
                _ => {}
            }
        }
        let trimmed = html.trim();
        let is_comment = trimmed.starts_with("<!--") && trimmed.ends_with("-->");
        if !is_comment {
            self.error(ErrorCode::E002, "HTML blocks are not supported", line);
        }
        Vec::new()
    }

    fn list(&mut self, start: Option<u64>, line: usize) -> Vec<Line> {
        let mut out: Vec<Line> = Vec::new();
        let mut number = start.unwrap_or(1);
        let ordered = start.is_some();
        let end = TagEnd::List(ordered);
        while let Some((event, _)) = self.events.get(self.pos).cloned() {
            let item_line = self.line_at(self.pos);
            self.pos += 1;
            match event {
                Event::End(e) if e == end => break,
                Event::Start(Tag::Item) => {
                    let content = self.blocks(Some(TagEnd::Item));
                    let marker = if ordered {
                        format!("{number}. ")
                    } else {
                        "- ".to_string()
                    };
                    number += 1;
                    out.extend(indent_item(&marker, content, item_line));
                }
                _ => {}
            }
        }
        if out.is_empty() {
            out.push(Line::new("#[]", line));
        }
        out
    }

    fn table(&mut self, aligns: Vec<pulldown_cmark::Alignment>, line: usize) -> Vec<Line> {
        let mut header = TableRow {
            cells: Vec::new(),
            line,
        };
        let mut rows: Vec<TableRow> = Vec::new();
        let mut current: Option<TableRow> = None;
        let mut in_head = false;

        while let Some((event, _)) = self.events.get(self.pos).cloned() {
            let at = self.line_at(self.pos);
            self.pos += 1;
            match event {
                Event::End(TagEnd::Table) => break,
                Event::Start(Tag::TableHead) => {
                    in_head = true;
                    header = TableRow {
                        cells: Vec::new(),
                        line: at,
                    };
                }
                Event::End(TagEnd::TableHead) => in_head = false,
                Event::Start(Tag::TableRow) => {
                    current = Some(TableRow {
                        cells: Vec::new(),
                        line: at,
                    })
                }
                Event::End(TagEnd::TableRow) => rows.extend(current.take()),
                Event::Start(Tag::TableCell) => {
                    let cell = self.table_cell();
                    if in_head {
                        if cell.plain.trim().is_empty() {
                            self.warn(WarningCode::W006, "empty table header cell", at);
                        }
                        header.cells.push(cell);
                    } else if let Some(row) = current.as_mut() {
                        row.cells.push(cell);
                    }
                }
                _ => {}
            }
        }

        let data = TableData {
            aligns,
            header,
            rows,
        };
        let (width, height) = self.text_area_pt;
        let mut layout = tables::plan(&data, width);
        tables::mark_tall_rows(&mut layout, &data, width, height);
        if layout.overflow {
            self.warn(
                WarningCode::W004,
                format!(
                    "table is wider than the page even at {} pt; it may overflow",
                    layout.font_size_pt
                ),
                line,
            );
        }
        tables::emit(&data, &layout)
            .into_iter()
            .map(|(t, l)| Line::new(t, l))
            .collect()
    }

    fn table_cell(&mut self) -> Cell {
        self.in_table = true;
        self.plain.clear();
        let markup = self.inline(Some(TagEnd::TableCell), Position::LineStart);
        self.in_table = false;
        Cell {
            markup,
            plain: std::mem::take(&mut self.plain),
        }
    }

    // ---- inline -------------------------------------------------------------------------

    fn flush_pending(
        &mut self,
        pending: &mut String,
        out: &mut String,
        position: &mut Position,
    ) {
        if !pending.is_empty() {
            let text = std::mem::take(pending);
            out.push_str(&self.escape_run(&text, *position));
            *position = Position::Inline;
        }
    }

    /// Render inline content until `end` (consumed). With `end == None`, stop (without
    /// consuming) at the first `End` or block-level `Start`, which is how tight list items
    /// and the top level are handled.
    fn inline(&mut self, end: Option<TagEnd>, start: Position) -> String {
        let mut out = String::new();
        let mut pending = String::new();
        let mut position = start;

        while let Some((event, _)) = self.events.get(self.pos).cloned() {
            let line = self.line_at(self.pos);
            match event {
                Event::End(e) if end == Some(e) => {
                    self.pos += 1;
                    break;
                }
                Event::End(_) if end.is_none() => break,
                Event::End(_) => self.pos += 1, // stray End inside a container: ignore
                Event::Start(ref tag) if end.is_none() && is_block_start(tag) => break,
                Event::Text(t) => {
                    self.pos += 1;
                    pending.push_str(&t);
                }
                Event::SoftBreak => {
                    self.pos += 1;
                    pending.push(' ');
                }
                Event::HardBreak => {
                    self.pos += 1;
                    self.flush_pending(&mut pending, &mut out, &mut position);
                    out.push_str("#linebreak()");
                    position = Position::AfterExpr;
                }
                Event::Code(c) => {
                    self.pos += 1;
                    self.flush_pending(&mut pending, &mut out, &mut position);
                    let mut code = c.to_string();
                    if self.in_table {
                        self.plain.push_str(&code);
                        code = tables::break_long_tokens(&code);
                    }
                    out.push_str(&format!("#raw({})", typst_string(&code)));
                    position = Position::AfterExpr;
                }
                Event::InlineHtml(_) | Event::Html(_) => {
                    self.pos += 1;
                    self.warn(
                        WarningCode::W002,
                        "inline HTML is not supported; it was dropped",
                        line,
                    );
                }
                Event::FootnoteReference(_) => {
                    self.pos += 1;
                    self.unsupported("footnotes", line);
                }
                Event::TaskListMarker(_) => {
                    self.pos += 1;
                    self.unsupported("task lists", line);
                }
                Event::InlineMath(_) | Event::DisplayMath(_) => {
                    self.pos += 1;
                    self.error(
                        ErrorCode::E002,
                        "math is not supported (dollar signs are literal)",
                        line,
                    );
                }
                Event::Rule => break,
                Event::Start(tag) => {
                    self.pos += 1;
                    self.flush_pending(&mut pending, &mut out, &mut position);
                    let rendered = self.inline_tag(tag, line);
                    let is_expr = rendered.starts_with('#');
                    out.push_str(&rendered);
                    position = if is_expr {
                        Position::AfterExpr
                    } else {
                        Position::Inline
                    };
                }
            }
        }
        self.flush_pending(&mut pending, &mut out, &mut position);
        out
    }

    fn escape_run(&mut self, text: &str, position: Position) -> String {
        if self.in_table {
            self.plain.push_str(text);
            escape_text(&tables::break_long_tokens(text), position)
        } else {
            escape_text(text, position)
        }
    }

    /// An inline `Start(tag)` has just been consumed; render it and its content.
    fn inline_tag(&mut self, tag: Tag<'a>, line: usize) -> String {
        match tag {
            Tag::Emphasis => {
                let inner = self.inline(Some(TagEnd::Emphasis), Position::LineStart);
                format!("#emph[{inner}]")
            }
            Tag::Strong => {
                let inner = self.inline(Some(TagEnd::Strong), Position::LineStart);
                format!("#strong[{inner}]")
            }
            Tag::Strikethrough => {
                self.unsupported_inline("strikethrough", TagEnd::Strikethrough, line)
            }
            Tag::Superscript => self.unsupported_inline("superscript", TagEnd::Superscript, line),
            Tag::Subscript => self.unsupported_inline("subscript", TagEnd::Subscript, line),
            Tag::Link { link_type, dest_url, .. } => self.link(link_type, &dest_url, line),
            Tag::Image { dest_url, .. } => self.inline_image(&dest_url, line),
            _ => {
                // Block-level tag in an inline context: not reachable with a well-formed
                // event stream; skip it rather than loop.
                self.skip_container();
                String::new()
            }
        }
    }

    fn inline_image(&mut self, dest: &str, line: usize) -> String {
        let alt = self.collect_plain_text();
        self.image_markup(dest, &alt, line)
    }

    fn unsupported_inline(&mut self, what: &str, end: TagEnd, line: usize) -> String {
        self.error(ErrorCode::E002, format!("{what} is not supported"), line);
        self.inline(Some(end), Position::LineStart)
    }

    fn link(&mut self, link_type: LinkType, dest: &str, line: usize) -> String {
        let visible = self.peek_plain_text();
        let descriptive_check = !matches!(link_type, LinkType::Autolink | LinkType::Email);
        if descriptive_check {
            let lowered = visible.trim().to_lowercase();
            if visible.trim() == dest || matches!(lowered.as_str(), "here" | "click here" | "this")
            {
                self.warn(
                    WarningCode::W009,
                    format!("link text `{}` is not descriptive", visible.trim()),
                    line,
                );
            }
        }
        let resolved = self.links.resolve(self.page, dest, line);
        let inner = self.inline(Some(TagEnd::Link), Position::LineStart);
        match resolved {
            Resolved::Internal { label } => format!("#link(<{label}>)[{inner}]"),
            Resolved::External { url, arrow: true } => {
                format!("#ext-link({})[{inner}]", typst_string(&url))
            }
            Resolved::External { url, arrow: false } => {
                format!("#link({})[{inner}]", typst_string(&url))
            }
            Resolved::File { path } => format!("#link({})[{inner}]", typst_string(&path)),
            Resolved::Unresolved => {
                // Plain text; neutralise a leading `#` run so the result is not an "expression".
                inner
            }
        }
    }
}

fn pulldown_level(level: u8) -> pulldown_cmark::HeadingLevel {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        1 => H1,
        2 => H2,
        3 => H3,
        4 => H4,
        5 => H5,
        _ => H6,
    }
}

/// Append a block, separated from the previous one by a blank line.
fn push_block(out: &mut Vec<Line>, block: Vec<Line>) {
    if block.is_empty() {
        return;
    }
    if let Some(last) = out.last() {
        let md_line = block[0].md_line;
        let _ = last;
        out.push(Line::new("", md_line));
    }
    out.extend(block);
}

/// Prefix the first line of an item with its marker and indent the rest to the marker width.
fn indent_item(marker: &str, content: Vec<Line>, line: usize) -> Vec<Line> {
    if content.is_empty() {
        return vec![Line::new(marker.trim_end(), line)];
    }
    let pad = " ".repeat(marker.len());
    content
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let text = if l.text.is_empty() {
                String::new()
            } else if i == 0 {
                format!("{marker}{}", l.text)
            } else {
                format!("{pad}{}", l.text)
            };
            Line::new(text, l.md_line)
        })
        .collect()
}

/// Alt text for a diagram: the quoted string after the fence language, else Mermaid's
/// `accTitle`/`accDescr` lines (plan 7.10 item 3).
fn diagram_alt(info_rest: &str, source: &str) -> Option<String> {
    let rest = info_rest.trim();
    if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
        let inner = rest[1..rest.len() - 1].trim();
        if !inner.is_empty() {
            return Some(inner.to_string());
        }
    }
    let find = |key: &str| {
        source.lines().find_map(|l| {
            let l = l.trim();
            l.strip_prefix(key)
                .and_then(|r| r.strip_prefix(':'))
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty())
        })
    };
    match (find("accTitle"), find("accDescr")) {
        (Some(t), Some(d)) => Some(format!("{t}. {d}")),
        (Some(t), None) => Some(t),
        (None, Some(d)) => Some(d),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::SourcePage;
    use crate::frontmatter::Frontmatter;
    use camino::Utf8PathBuf;
    use std::path::Path;

    fn page(body: &str) -> Page {
        let src = SourcePage {
            tier: "t".into(),
            rel_path: "t/p.md".into(),
            abs_path: Utf8PathBuf::from("t/p.md"),
            frontmatter: Frontmatter {
                body_line_offset: 0,
                ..Default::default()
            },
            body: body.to_string(),
        };
        Page::build(src, "t-p".into()).unwrap()
    }

    fn emit(body: &str) -> Emitted {
        let p = page(body);
        let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
        emit_page(&p, &mut links, &mut NoAssets, (453.5, 700.0))
    }

    fn md(body: &str) -> String {
        let e = emit(body);
        assert!(!e.diag.has_errors(), "{:?}", e.diag.errors);
        e.markup.text
    }

    fn errors(body: &str) -> Vec<HideoError> {
        emit(body).diag.errors
    }

    #[test]
    fn paragraphs_emphasis_and_breaks() {
        assert_eq!(
            md("Hello *world* and **bold**.\n"),
            "Hello #emph[world] and #strong[bold]."
        );
        assert_eq!(md("one\ntwo\n"), "one two");
        assert_eq!(md("one  \ntwo\n"), "one#linebreak()two");
        assert_eq!(md("a\n\nb\n"), "a\n\nb");
        assert_eq!(md("foo**bar**baz"), "foo#strong[bar]baz");
    }

    #[test]
    fn escapes_text_and_protects_after_expressions() {
        assert_eq!(
            md("50% of #1 costs $5 @home"),
            "50% of \\#1 costs \\$5 \\@home"
        );
        assert_eq!(md("**bold**.com"), "#strong[bold]\\.com");
        assert_eq!(md("**bold**(x)"), "#strong[bold]\\(x)");
        assert_eq!(md("`code`.name"), "#raw(\"code\")\\.name");
    }

    #[test]
    fn line_start_markers_are_escaped() {
        assert_eq!(md("\\= not heading"), "\\= not heading");
        assert_eq!(md("1\\. not a list"), "1\\. not a list");
        assert_eq!(md("\\- not a list"), "\\- not a list");
        assert_eq!(md("\\+ not a list"), "\\+ not a list");
    }

    #[test]
    fn headings_carry_page_qualified_labels() {
        assert_eq!(
            md("# Title\n\n## Sub {#custom}\n"),
            "#heading(level: 1)[Title] <t-p--title>\n\n#heading(level: 2)[Sub] <t-p--custom>"
        );
    }

    #[test]
    fn skipped_heading_level_warns() {
        let e = emit("# A\n\n### C\n");
        assert!(
            e.diag
                .warnings
                .iter()
                .any(|w| w.code == WarningCode::W007 && w.line == Some(3))
        );
        let e = emit("# A\n\n## B\n\n### C\n\n## D\n");
        assert!(e.diag.warnings.iter().all(|w| w.code != WarningCode::W007));
    }

    #[test]
    fn code_is_a_string_literal() {
        assert_eq!(
            md("```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n"),
            "#raw(\"fn main() {\\n    println!(\\\"hi\\\");\\n}\", block: true, lang: \"rust\")"
        );
        assert_eq!(
            md("```\n``` ` ```\n```\n"),
            "#raw(\"``` ` ```\", block: true)"
        );
        assert_eq!(md("    indented\n"), "#raw(\"indented\", block: true)");
        assert_eq!(md("Use `a`b`c` here"), "Use #raw(\"a\")b#raw(\"c\") here");
    }

    #[test]
    fn invalid_code_language_is_dropped_with_warning() {
        let e = emit("```ru\"st\ncode\n```\n");
        assert!(
            e.markup.text.starts_with("#raw(\"code\", block: true)"),
            "{}",
            e.markup.text
        );
        assert!(e.diag.warnings.iter().any(|w| w.code == WarningCode::W010));
    }

    #[test]
    fn lists_nest_and_number() {
        assert_eq!(md("- a\n- b\n  - c\n"), "- a\n- b\n\n  - c");
        assert_eq!(md("1. a\n2. b\n"), "1. a\n2. b");
        assert_eq!(md("3. a\n4. b\n"), "3. a\n4. b");
        assert_eq!(md("- a\n\n  para\n- b\n"), "- a\n\n  para\n- b");
        assert_eq!(md("10. x\n11. y\n    more\n"), "10. x\n11. y more");
    }

    #[test]
    fn list_item_text_that_looks_like_marker_is_escaped() {
        assert_eq!(md("- \\- x\n"), "- \\- x");
        assert_eq!(md("- 1\\. x\n"), "- 1\\. x");
    }

    #[test]
    fn quotes_and_rules() {
        assert_eq!(md("> a\n>\n> b\n"), "#quote(block: true)[\na\n\nb\n]");
        assert_eq!(md("a\n\n---\n\nb\n"), "a\n\n#hr()\n\nb");
        assert_eq!(md("> - x\n"), "#quote(block: true)[\n- x\n]");
    }

    #[test]
    fn lead_in_paragraph() {
        assert_eq!(md("**Note.** Rest of it."), "#lead[Note.] Rest of it.");
        assert_eq!(md("**A** **B.** rest"), "#strong[A] #strong[B.] rest");
        assert_eq!(md("**Note.** .com"), "#lead[Note.] .com");
        assert_eq!(md("**Note.**.com"), "#lead[Note.]\\.com");
    }

    #[test]
    fn links_are_resolved_and_marked() {
        assert_eq!(
            md("[x](https://a.example)"),
            "#ext-link(\"https://a.example\")[x]"
        );
        assert_eq!(md("[mail](mailto:a@b.c)"), "#link(\"mailto:a@b.c\")[mail]");
        assert_eq!(
            md("# H\n\n[up](#h)"),
            "#heading(level: 1)[H] <t-p--h>\n\n#link(<t-p--h>)[up]"
        );
        // Unresolved: plain text, reported by the link map.
        let p = page("[gone](nope.md)");
        let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
        let e = emit_page(&p, &mut links, &mut NoAssets, (453.5, 700.0));
        assert_eq!(e.markup.text, "gone");
        assert_eq!(links.unresolved.len(), 1);
        assert_eq!(links.unresolved[0].line, 1);
    }

    #[test]
    fn external_link_text_is_url_warns() {
        let e = emit(
            "[https://a.example](https://a.example) and [here](https://b.example) and <https://c.example>",
        );
        let w: Vec<_> = e
            .diag
            .warnings
            .iter()
            .filter(|w| w.code == WarningCode::W009)
            .collect();
        assert_eq!(w.len(), 2, "{w:?}");
    }

    #[test]
    fn unsupported_constructs_are_errors_with_page_and_line() {
        for (md, what) in [
            ("a\n\n~~gone~~\n", "strikethrough"),
            ("a\n\nnote[^1]\n\n[^1]: text\n", "footnotes"),
            ("a\n\n- [ ] todo\n", "task lists"),
            ("a\n\nTerm\n: Definition\n", "definition lists"),
            ("a\n\nx ^2^ y\n", "superscript"),
            ("a\n\nH ~2~ O\n", "subscript"),
            ("a\n\n<div>\nhtml\n</div>\n", "HTML blocks"),
        ] {
            let errs = errors(md);
            assert!(!errs.is_empty(), "{what}: expected an error for {md:?}");
            assert!(
                errs.iter().all(|e| e.code == ErrorCode::E002),
                "{what}: {errs:?}"
            );
            assert!(
                errs.iter().any(|e| e.message.contains(what)),
                "{what}: {errs:?}"
            );
            assert!(
                errs.iter()
                    .all(|e| e.page.as_deref() == Some("t/p.md") && e.line.is_some())
            );
            assert!(
                errs[0].line.unwrap() >= 3,
                "{what}: line {:?}",
                errs[0].line
            );
        }
    }

    #[test]
    fn inline_html_warns_and_is_dropped() {
        let e = emit("a <b>bold</b> c");
        assert!(!e.diag.has_errors());
        assert_eq!(e.markup.text, "a bold c");
        assert_eq!(
            e.diag
                .warnings
                .iter()
                .filter(|w| w.code == WarningCode::W002)
                .count(),
            2
        );
    }

    #[test]
    fn html_comment_dropped_silently() {
        let e = emit("a\n\n<!-- hidden -->\n\nb\n");
        assert!(e.diag.errors.is_empty() && e.diag.warnings.is_empty());
        assert_eq!(e.markup.text, "a\n\nb");
        let e = emit("a\n\n<!--\nmulti\nline\n-->\n\nb\n");
        assert!(e.diag.errors.is_empty(), "{:?}", e.diag.errors);
    }

    #[test]
    fn dollar_is_literal() {
        assert_eq!(md("$x$ and $$y$$"), "\\$x\\$ and \\$\\$y\\$\\$");
    }

    #[test]
    fn tables_use_header_function() {
        let out = md("| A | B |\n|---|--:|\n| 1 | 2 |\n| **x** | `y` |\n");
        assert!(out.contains("table.header([A], [B]),"), "{out}");
        assert!(out.contains("align: (left, right,),"));
        assert!(out.contains("[1], [2],"));
        assert!(out.contains("[#strong[x]], [#raw(\"y\")],"));
        assert!(out.starts_with("#[\n#set text(size: 8.5pt)\n#table("));
    }

    #[test]
    fn empty_table_header_cell_warns() {
        let e = emit("| A |  |\n|---|---|\n| 1 | 2 |\n");
        assert!(
            e.diag
                .warnings
                .iter()
                .any(|w| w.code == WarningCode::W006 && w.line == Some(1))
        );
    }

    #[test]
    fn table_cells_break_long_tokens() {
        let out = md("| url |\n|---|\n| https://example.com/a/very/long/path |\n");
        assert!(out.contains('\u{200B}'));
    }

    #[test]
    fn images_become_block_images() {
        assert_eq!(
            md("![A cat](cat.png)"),
            "#hideo-image(\"images/cat.png\", alt: \"A cat\")"
        );
        assert_eq!(
            md("![](deco.png)"),
            "#hideo-image(\"images/deco.png\", alt: none)"
        );
    }

    #[test]
    fn diagrams_get_alt_text() {
        assert_eq!(
            md("```mermaid \"Flow of data\"\ngraph TD; A-->B\n```\n"),
            "#hideo-diagram(\"diagrams/test.svg\", alt: \"Flow of data\")"
        );
        assert_eq!(
            md(
                "```mermaid\ngraph TD\n  accTitle: Pipeline\n  accDescr: Shows steps\n  A-->B\n```\n"
            ),
            "#hideo-diagram(\"diagrams/test.svg\", alt: \"Pipeline. Shows steps\")"
        );
        let e = emit("```mermaid\ngraph TD; A-->B\n```\n");
        assert_eq!(
            e.markup.text,
            "#hideo-diagram(\"diagrams/test.svg\", alt: \"Diagram\")"
        );
        assert!(
            e.diag
                .warnings
                .iter()
                .any(|w| w.code == WarningCode::W008 && w.line == Some(1))
        );
    }

    #[test]
    fn line_map_points_at_markdown_lines() {
        let e = emit("first\n\n## Second\n\n- a\n- b\n");
        let text: Vec<&str> = e.markup.text.split('\n').collect();
        assert_eq!(text.len(), e.markup.line_map.len());
        let line_of = |needle: &str| {
            let i = text.iter().position(|l| l.contains(needle)).unwrap();
            e.markup.line_map[i]
        };
        assert_eq!(line_of("first"), 1);
        assert_eq!(line_of("Second"), 3);
        assert_eq!(line_of("- a"), 5);
        assert_eq!(line_of("- b"), 6);
    }

    /// Every construct of the profile in one document, as a reviewed snapshot (plan test
    /// level 2). Review changes with `cargo insta review` or `INSTA_UPDATE=always`.
    #[test]
    fn test_markdown_profile_all_constructs() {
        let md = "# Title {#top}

A paragraph with *emphasis*, **strong**, `code`, a [web link](https://example.com/a), a
[mail link](mailto:a@b.c), an [internal link](#top) and a hard  
break. Special characters: # $ @ * _ < > [ ] ~ \\ and 5% of 2020-2021, COVID-19.

**Note.** A lead-in paragraph.

## Lists

- bullet
  - nested bullet
    1. numbered in nested

3. starts at three
4. four

> A quote with a [link](#top).
>
> - and a list

---

```rust
fn main() { println!(\"hi\"); }
```

    indented code

![A figure](figure.png)

| Left | Centre | Right |
|:-----|:------:|------:|
| a | **b** | `c` |
| long https://example.com/a/very/long/path | x | y |

```mermaid \"A diagram\"
graph TD; A-->B
```

<!-- a comment is dropped -->

Inline <b>html</b> is dropped with a warning.
";
        let e = emit(md);
        assert!(!e.diag.has_errors(), "{:?}", e.diag.errors);
        insta::assert_snapshot!("profile_markup", e.markup.text);
        let warnings: Vec<String> = e.diag.warnings.iter().map(|w| w.to_string()).collect();
        insta::assert_snapshot!(
            "profile_warnings",
            warnings.join(
                "
"
            )
        );
    }

    #[test]
    fn test_parser_panic_reported_with_page() {
        struct Panicking;
        impl AssetSink for Panicking {
            fn body_image(&mut self, _p: &Page, _d: &str, _l: usize) -> Option<String> {
                panic!("simulated parser failure");
            }
            fn diagram(&mut self, _p: &Page, _s: &str, _l: usize) -> Option<String> {
                None
            }
        }
        let p = page(
            "![x](y.png)
",
        );
        let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
        let result = emit_page(&p, &mut links, &mut Panicking, (453.5, 700.0));
        assert_eq!(result.diag.errors.len(), 1);
        assert_eq!(result.diag.errors[0].code, ErrorCode::E005);
        assert_eq!(result.diag.errors[0].page.as_deref(), Some("t/p.md"));
        assert!(result.markup.text.is_empty());
    }

    #[test]
    fn pathological_input_parses_fast() {
        let mut body = String::new();
        body.push_str(&"*a_".repeat(3000));
        body.push('\n');
        body.push_str(&"[^\n".repeat(3000));
        body.push_str(&"\n".repeat(5000));
        let start = std::time::Instant::now();
        let _ = emit(&body);
        assert!(start.elapsed().as_secs_f64() < 5.0, "{:?}", start.elapsed());
    }

    #[test]
    fn test_diagram_alt_description_only() {
        assert_eq!(
            diagram_alt("", "accDescr: only desc\ngraph TD"),
            Some("only desc".to_string())
        );
        assert_eq!(
            diagram_alt("  \"custom alt\"  ", "graph TD"),
            Some("custom alt".to_string())
        );
    }

    #[test]
    fn test_heading_level_6() {
        assert!(md("###### Deep heading").starts_with("#heading(level: 6)"));
    }

    #[test]
    fn test_empty_list_item() {
        assert_eq!(md("- \n- a\n"), "-\n- a");
    }

    #[test]
    fn test_table_with_code_and_overflow() {
        let e = emit("| h |\n| --- |\n| `abc` |");
        assert!(e.markup.text.contains("#raw(\"abc\")"));

        let wide = "| aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | cccccccccccccccccccccccccccccccccc | dddddddddddddddddddddddddddddddddd | eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | ffffffffffffffffffffffffffffffffff |\n|---|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 | 6 |";
        let e_wide = emit(wide);
        assert!(e_wide.diag.warnings.iter().any(|w| w.code == WarningCode::W004));
    }

    #[test]
    fn test_inline_image_and_formatted_alt() {
        assert_eq!(
            md("text before ![alt](img.png) text after"),
            "text before #hideo-image(\"images/img.png\", alt: \"alt\") text after"
        );
        assert_eq!(
            md("![**bold** alt\nbreak](img.png)"),
            "#hideo-image(\"images/img.png\", alt: \"bold alt break\")"
        );
        assert_eq!(
            md("![\t](img.png)"),
            "#hideo-image(\"images/img.png\", alt: none)"
        );
    }

    struct NoneAssetSink;
    impl AssetSink for NoneAssetSink {
        fn body_image(&mut self, _page: &Page, _dest: &str, _line: usize) -> Option<String> {
            None
        }
        fn diagram(&mut self, _page: &Page, _source: &str, _line: usize) -> Option<String> {
            None
        }
    }

    #[test]
    fn test_missing_image_sink_and_file_link() {
        let p = page("![fallback](missing.png)");
        let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
        let e = emit_page(&p, &mut links, &mut NoneAssetSink, (453.5, 700.0));
        assert_eq!(e.markup.text, "fallback");

        // Resolved::File test
        let dir = tempfile::tempdir().unwrap();
        let t_dir = dir.path().join("t");
        std::fs::create_dir_all(&t_dir).unwrap();
        std::fs::write(t_dir.join("doc.pdf"), b"pdf").unwrap();
        let p2 = page("[doc](doc.pdf)");
        let mut links2 = LinkMap::new("t", std::slice::from_ref(&p2), [], dir.path());
        let e2 = emit_page(&p2, &mut links2, &mut NoAssets, (453.5, 700.0));
        assert_eq!(e2.markup.text, "#link(\"doc.pdf\")[doc]");
    }

    #[test]
    fn test_tight_list_item_with_empty_inline() {
        let e = emit("- <!-- comment -->\n- real\n");
        assert!(e.markup.text.contains("- real"));
    }

    #[test]
    fn test_emitter_edge_cases() {
        let p = page("body text");
        let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
        let mut assets = NoAssets;
        let mut emitter = Emitter::new(&p, &mut links, &mut assets, (453.5, 700.0));

        // 1. line_at past end of events (line 183)
        assert_eq!(emitter.line_at(9999), p.byte_to_line(p.body.len()));

        // 2. heading with index past p.headings.len() (line 451)
        emitter.heading_index = 100;
        let h_lines = emitter.heading(1, 1);
        assert!(h_lines[0].text.contains("<t-p--h100>"));

        // 3. collect_plain_text with HardBreak and non-text event (line 239)
        emitter.events = vec![
            (Event::Text("word".into()), 0..4),
            (Event::Rule, 4..5),
            (Event::HardBreak, 5..6),
            (Event::End(TagEnd::Emphasis), 6..7),
        ];
        emitter.pos = 0;
        let text = emitter.collect_plain_text();
        assert_eq!(text, "word ");

        // 4. peek_plain_text with Start and _ (lines 251, 259)
        emitter.events = vec![
            (Event::Start(Tag::Emphasis), 0..1),
            (Event::SoftBreak, 1..2),
            (Event::End(TagEnd::Emphasis), 2..3),
        ];
        emitter.pos = 0;
        let peeked = emitter.peek_plain_text();
        assert_eq!(peeked, "");

        // 5. inline with math, rule, stray End, block start (lines 695, 698, 740-748)
        emitter.events = vec![
            (Event::InlineMath("x".into()), 0..1),
            (Event::DisplayMath("y".into()), 1..2),
            (Event::End(TagEnd::Emphasis), 2..3),
            (Event::Start(Tag::TableHead), 3..4),
            (Event::End(TagEnd::TableHead), 4..5),
            (Event::Rule, 5..6),
        ];
        emitter.pos = 0;
        let _ = emitter.inline(Some(TagEnd::Paragraph), Position::LineStart);
        assert!(emitter.diag.has_errors());

        // 5b. inline stopping at block start when end is None (line 698)
        emitter.events = vec![(Event::Start(Tag::Paragraph), 0..1)];
        emitter.pos = 0;
        let _ = emitter.inline(None, Position::LineStart);

        // 6. code_block with non-text event (line 466)
        emitter.events = vec![
            (Event::SoftBreak, 0..1),
            (Event::End(TagEnd::CodeBlock), 1..2),
        ];
        emitter.pos = 0;
        let cb_lines = emitter.code_block(CodeBlockKind::Fenced("".into()), 1);
        assert!(!cb_lines.is_empty());

        // 7. html_block with non-html event (line 542)
        emitter.events = vec![
            (Event::Text("non-html".into()), 0..1),
            (Event::End(TagEnd::HtmlBlock), 1..2),
        ];
        emitter.pos = 0;
        let hb_lines = emitter.html_block(1);
        assert!(hb_lines.is_empty());

        // 8. list with non-item event and empty list (lines 573, 577)
        emitter.events = vec![
            (Event::SoftBreak, 0..1),
            (Event::End(TagEnd::List(false)), 1..2),
        ];
        emitter.pos = 0;
        let list_lines = emitter.list(None, 1);
        assert_eq!(list_lines[0].text, "#[]");

        // 9. table with non-cell event (line 622)
        emitter.events = vec![
            (Event::SoftBreak, 0..1),
            (Event::End(TagEnd::Table), 1..2),
        ];
        emitter.pos = 0;
        let tbl_lines = emitter.table(vec![], 1);
        assert!(!tbl_lines.is_empty());

        // 10. blocks with stray HTML outside container (lines 288, 290)
        emitter.events = vec![(Event::Html("<div></div>".into()), 0..11)];
        emitter.pos = 0;
        let blk_lines = emitter.blocks(None);
        assert!(blk_lines.is_empty());

        // 11. metadata block in blocks (lines 334-336)
        emitter.events = vec![(Event::End(TagEnd::MetadataBlock(pulldown_cmark::MetadataBlockKind::YamlStyle)), 0..1)];
        emitter.pos = 0;
        let meta_lines = emitter.block(Tag::MetadataBlock(pulldown_cmark::MetadataBlockKind::YamlStyle), 1);
        assert!(meta_lines.is_empty());

        // 12. block with inline tag rendering empty (line 343)
        emitter.events = vec![
            (Event::End(TagEnd::TableCell), 0..1),
        ];
        emitter.pos = 1;
        let empty_inline_block = emitter.block(Tag::TableCell, 1);
        assert!(empty_inline_block.is_empty());

        // 13. blockquote with kind (line 312)
        assert_eq!(md("> [!NOTE]\n> alert\n"), "#quote(block: true)[\n\\[!NOTE\\] alert\n]");
        emitter.events = vec![
            (Event::End(TagEnd::BlockQuote(Some(pulldown_cmark::BlockQuoteKind::Note))), 0..1),
        ];
        emitter.pos = 0;
        let bq = emitter.block(Tag::BlockQuote(Some(pulldown_cmark::BlockQuoteKind::Note)), 1);
        assert!(!bq.is_empty());

        // 14. blocks with mismatched End event (line 273)
        emitter.events = vec![
            (Event::End(TagEnd::Emphasis), 0..1),
            (Event::End(TagEnd::Item), 1..2),
        ];
        emitter.pos = 0;
        let _ = emitter.blocks(Some(TagEnd::Item));
    }

    // ---- property: link destinations never produce uncompilable Typst (plan 8.3) ---------

    mod link_property {
        use super::*;
        use crate::compile::{compile_document, extract_text};
        use crate::world::HideoWorld;
        use proptest::prelude::*;
        use typst::foundations::Datetime;

        /// Minimal stand-ins for the template helpers the emitter calls, plus the page anchor.
        const PRELUDE: &str = "#set page(width: 300pt, height: auto)\n\
#let ext-link(url, body) = link(url, body)\n\
#let lead(body) = body\n\
#metadata(none) <t-p>\n";

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(300))]

            #[test]
            fn any_link_destination_emits_compilable_typst(
                dest in "[a-zA-Z0-9 _.:/#%\\?&=@~-]{0,30}",
                label in "[a-z]{1,8}",
            ) {
                // `<...>` allows spaces in the destination; strip characters that cannot be
                // part of a Markdown destination at all.
                let dest: String = dest.chars().filter(|c| !matches!(c, '<' | '>' | '\\')).collect();
                let body = format!("# Heading\n\nText [{label}](<{dest}>) and [again](<#heading>) end.\n");
                let p = page(&body);
                let mut links = LinkMap::new("t", std::slice::from_ref(&p), [], Path::new("nowhere"));
                let emitted = emit_page(&p, &mut links, &mut NoAssets, (453.5, 700.0));
                prop_assert!(!emitted.diag.has_errors(), "{:?}", emitted.diag.errors);
                let src = format!("{PRELUDE}{}", emitted.markup.text);
                let world = HideoWorld::from_source(&src, Datetime::from_ymd(2000, 1, 1).unwrap());
                let warned = compile_document(&world);
                match warned.output {
                    Ok(doc) => {
                        let text = extract_text(&doc).concat();
                        prop_assert!(text.contains("Heading"), "{}", text);
                    }
                    Err(e) => prop_assert!(false, "does not compile: {:?}\n{}", e, src),
                }
            }
        }
    }
}
