//! Markdown profile and Typst emission (plan section 5 and 7.5).

pub mod emit;
pub mod escape;
pub mod lead;
pub mod tables;

use pulldown_cmark::Options;

/// Parser options (plan section 5). Tables and heading attributes are real features; the other
/// extensions are enabled **only to detect them** and raise an error naming page and line.
/// Smart punctuation, math and metadata blocks stay off.
pub fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
}
