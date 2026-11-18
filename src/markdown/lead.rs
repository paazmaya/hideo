use pulldown_cmark::{Event, Tag, TagEnd};
use crate::markdown::escape::escape_text;
use crate::markdown::escape::Position;

pub fn is_lead_paragraph(events: &[Event]) -> bool {
    if events.is_empty() {
        return false;
    }

    let mut iter = events.iter().peekable();

    if !matches!(iter.next(), Some(Event::Start(Tag::Strong))) {
        return false;
    }

    let mut strong_text = String::new();
    let mut text_count = 0;

    while let Some(event) = iter.next() {
        match event {
            Event::Text(t) | Event::Code(t) => {
                strong_text.push_str(t);
                text_count += 1;
            }
            Event::End(TagEnd::Strong) => break,
            Event::Start(Tag::Link { .. }) | Event::Start(Tag::Image { .. }) | Event::Start(Tag::CodeBlock) => {
                return false;
            }
            _ => {}
        }
    }

    if text_count != 1 {
        return false;
    }

    let trimmed = strong_text.trim_end();
    if !trimmed.ends_with('.') && !trimmed.ends_with(':') {
        return false;
    }

    if trimmed.len() > 40 {
        return false;
    }

    true
}

pub fn emit_lead(strong_text: &str, rest_events: &[Event]) -> String {
    let mut out = String::new();
    out.push_str(&format!("#lead[{}", escape_text(strong_text, Position::Inline)));
    out.push_str("]\n");

    for event in rest_events {
        out.push_str(&emit_event(event, Position::Inline));
    }

    out
}

fn emit_event(event: &Event, position: Position) -> String {
    match event {
        Event::Text(t) => escape_text(t, position),
        Event::Code(t) => format!("`{}`", escape_text(t, position)),
        Event::SoftBreak => " ".to_string(),
        Event::HardBreak => "\\\n".to_string(),
        Event::Start(Tag::Emphasis) => "*".to_string(),
        Event::End(TagEnd::Emphasis) => "*".to_string(),
        Event::Start(Tag::Strong) => "**".to_string(),
        Event::End(TagEnd::Strong) => "**".to_string(),
        Event::Start(Tag::Link { dest_url, .. }) => format!("#link(\"{}\")[", escape_typst_string(dest_url)),
        Event::End(TagEnd::Link) => "]".to_string(),
        _ => String::new(),
    }
}

fn escape_typst_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}