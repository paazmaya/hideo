use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{code}: {message}")]
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

    pub fn with_location(mut self, page: impl Into<String>, line: usize) -> Self {
        self.page = Some(page.into());
        self.line = Some(line);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorCode {
    E001, // Frontmatter
    E002, // Unsupported construct
    E003, // Bad heading id
    E004, // Diagram
    E005, // Compile
    E006, // PDF/UA validation failure
    E007, // Configuration
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

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

    pub fn with_location(mut self, page: impl Into<String>, line: usize) -> Self {
        self.page = Some(page.into());
        self.line = Some(line);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WarningCode {
    W001, // Unresolved link
    W002, // Inline HTML
    W003, // Missing hero
    W004, // Table overflow
    W005, // Case mismatch
    W006, // Empty table header cell
    W007, // Skipped heading level
    W008, // Missing diagram alt text
}

impl fmt::Display for WarningCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Default)]
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

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn sort(&mut self) {
        self.errors.sort_by(|a, b| {
            a.page.cmp(&b.page).then_with(|| a.line.cmp(&b.line)).then_with(|| a.code.cmp(&b.code))
        });
        self.warnings.sort_by(|a, b| {
            a.page.cmp(&b.page).then_with(|| a.line.cmp(&b.line)).then_with(|| a.code.cmp(&b.code))
        });
    }
}