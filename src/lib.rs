//! hideo - A standalone Rust CLI for building PDF books from Markdown

pub mod cli;
pub mod config;
pub mod error;
pub mod discover;
pub mod frontmatter;
pub mod page;
pub mod linkmap;
pub mod markdown;
pub mod images;
pub mod diagrams;
pub mod template;
pub mod world;
pub mod compile;
pub mod report;

#[cfg(test)]
mod tests {
    #[test]
    fn test_placeholder() {
        assert!(true);
    }
}