//! Error and warning types.
//!
//! Every diagnostic carries a stable code, an optional page and an optional line so that
//! `hideo check --format json` and the tests can assert on them (plan section 4.1, F23).

use std::fmt;

use thiserror::Error;

/// A content error: the build cannot produce a correct PDF.
#[derive(Debug, Clone, Error)]
#[error("{}", self.render())]
pub struct HideoError {
    pub code: ErrorCode,
    pub page: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

impl HideoError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            page: None,
            line: None,
            message: message.into(),
        }
    }

    pub fn with_page(mut self, page: impl Into<String>) -> Self {
        self.page = Some(page.into());
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_location(self, page: impl Into<String>, line: usize) -> Self {
        self.with_page(page).with_line(line)
    }

    fn render(&self) -> String {
        format!(
            "error[{}]: {}{}",
            self.code,
            location(&self.page, self.line),
            self.message
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorCode {
    /// Frontmatter, unreadable page, non-UTF-8 path.
    E001,
    /// Unsupported Markdown construct.
    E002,
    /// Bad heading id.
    E003,
    /// Diagram.
    E004,
    /// Typst compile error or parser panic.
    E005,
    /// PDF/UA validation failure.
    E006,
    /// Configuration.
    E007,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// A warning: the PDF is still produced, but the author should look.
#[derive(Debug, Clone)]
pub struct Warning {
    pub code: WarningCode,
    pub page: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

impl Warning {
    pub fn new(code: WarningCode, message: impl Into<String>) -> Self {
        Self {
            code,
            page: None,
            line: None,
            message: message.into(),
        }
    }

    pub fn with_page(mut self, page: impl Into<String>) -> Self {
        self.page = Some(page.into());
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_location(self, page: impl Into<String>, line: usize) -> Self {
        self.with_page(page).with_line(line)
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "warning[{}]: {}{}",
            self.code,
            location(&self.page, self.line),
            self.message
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarningCode {
    /// Unresolved link.
    W001,
    /// Inline HTML dropped.
    W002,
    /// Missing hero image (placeholder used), or other missing image.
    W003,
    /// Table overflows the text width.
    W004,
    /// Link case differs from the file on disk.
    W005,
    /// Empty table header cell.
    W006,
    /// Skipped heading level.
    W007,
    /// Diagram without alt text.
    W008,
    /// Link text is the URL itself or a non-descriptive word.
    W009,
    /// Other notice: skipped tier, dropped code language, diagrams off, Typst warnings.
    W010,
    /// Characters the embedded fonts cannot display.
    W011,
}

impl fmt::Display for WarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

fn location(page: &Option<String>, line: Option<usize>) -> String {
    match (page, line) {
        (Some(p), Some(l)) => format!("{p}:{l}: "),
        (Some(p), None) => format!("{p}: "),
        _ => String::new(),
    }
}

/// Collects errors and warnings across the pipeline.
#[derive(Debug, Default, Clone)]
pub struct DiagnosticCollector {
    pub errors: Vec<HideoError>,
    pub warnings: Vec<Warning>,
}

impl DiagnosticCollector {
    pub fn add_error(&mut self, error: HideoError) {
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }

    pub fn extend(&mut self, other: DiagnosticCollector) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Sort by page, line, code so that output does not depend on thread timing (F19).
    pub fn sort(&mut self) {
        self.errors.sort_by(|a, b| {
            (&a.page, a.line, a.code, &a.message).cmp(&(&b.page, b.line, b.code, &b.message))
        });
        self.warnings.sort_by(|a, b| {
            (&a.page, a.line, a.code, &a.message).cmp(&(&b.page, b.line, b.code, &b.message))
        });
    }
}

/// A usage or configuration problem: process exit code 2 (plan 7.3).
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ConfigError(pub String);
