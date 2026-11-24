//! hideo: a standalone Rust CLI for building PDF books from Markdown.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod app;
pub mod build;
pub mod cli;
pub mod compile;
pub mod config;
pub mod diagrams;
pub mod discover;
pub mod error;
pub mod frontmatter;
pub mod images;
pub mod linkmap;
pub mod markdown;
pub mod page;
pub mod report;
pub mod template;
pub mod world;
