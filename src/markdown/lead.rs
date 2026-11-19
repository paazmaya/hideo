//! Lead-in paragraphs (plan F17): a paragraph that opens with a short bold label such as
//! `**Note.** The rest of the paragraph.`

use pulldown_cmark::{Event, Tag, TagEnd};

/// Maximum length of the label in characters.
pub const MAX_LEAD_CHARS: usize = 40;

/// If the paragraph events (the events *inside* `Start(Paragraph)`/`End(Paragraph)`) open with a
/// lead-in label, return the label text and the number of events it occupies (always 3).
///
/// Qualifies: `Start(Strong)`, exactly one `Text`, `End(Strong)`, where the text ends in `.` or
/// `:` and is at most 40 characters (a 40-character text ending in `.` qualifies). Strong text
/// containing code, links or other inline elements does not qualify, and `**A** **B.**` is two
/// separate strong runs, of which only the first is looked at.
pub fn detect_lead<'a>(events: &'a [Event<'a>]) -> Option<(&'a str, usize)> {
    if let (
        Some(Event::Start(Tag::Strong)),
        Some(Event::Text(text)),
        Some(Event::End(TagEnd::Strong)),
    ) = (events.first(), events.get(1), events.get(2))
    {
        let trimmed = text.trim_end();
        let ends_ok = trimmed.ends_with('.') || trimmed.ends_with(':');
        (ends_ok && text.chars().count() <= MAX_LEAD_CHARS).then_some((&**text, 3))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::parser_options;
    use pulldown_cmark::Parser;

    fn lead_of(md: &str) -> Option<String> {
        let events: Vec<Event> = Parser::new_ext(md, parser_options()).collect();
        // events[0] is Start(Paragraph)
        detect_lead(&events[1..]).map(|(t, _)| t.to_string())
    }

    #[test]
    fn test_lead_in_label() {
        assert_eq!(lead_of("**Note.** Rest of it"), Some("Note.".into()));
        assert_eq!(lead_of("**Example:** more"), Some("Example:".into()));
        assert_eq!(lead_of("**Only.**"), Some("Only.".into()));
    }

    #[test]
    fn test_lead_in_length_limit() {
        let forty = format!("{}.", "a".repeat(39));
        assert_eq!(forty.chars().count(), 40);
        assert_eq!(lead_of(&format!("**{forty}** x")), Some(forty));
        let forty_one = format!("{}.", "a".repeat(40));
        assert_eq!(lead_of(&format!("**{forty_one}** x")), None);
    }

    #[test]
    fn test_lead_in_not_when_split_strong() {
        assert_eq!(lead_of("**A** **B.** rest"), None);
        assert_eq!(lead_of("**No terminator** rest"), None);
        assert_eq!(lead_of("plain **Note.** later"), None);
    }

    #[test]
    fn test_lead_in_not_with_code_or_link() {
        assert_eq!(lead_of("**`code`.** rest"), None);
        assert_eq!(lead_of("**[link](x.md).** rest"), None);
        assert_eq!(lead_of("**Some *nested*.** rest"), None);
        assert_eq!(lead_of("*Emphasis.* rest"), None);
    }
}
