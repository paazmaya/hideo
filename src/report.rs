//! Reports: text for people, JSON for tools (plan F8, F23).

use std::collections::BTreeMap;
use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::error::DiagnosticCollector;
use crate::linkmap::{UnresolvedLink, group_unresolved};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportItem {
    pub code: String,
    pub page: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stats {
    pub tiers: usize,
    pub pages: usize,
    pub images: usize,
    pub diagrams: usize,
    pub pdfs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub errors: Vec<ReportItem>,
    pub warnings: Vec<ReportItem>,
    pub stats: Stats,
}

impl Report {
    /// Unresolved links become `W001` warnings so JSON consumers see everything in one place.
    pub fn new(diag: &DiagnosticCollector, unresolved: &[UnresolvedLink], stats: Stats) -> Self {
        let errors = diag
            .errors
            .iter()
            .map(|e| ReportItem {
                code: e.code.to_string(),
                page: e.page.clone(),
                line: e.line,
                message: e.message.clone(),
            })
            .collect();
        let mut warnings: Vec<ReportItem> = diag
            .warnings
            .iter()
            .map(|w| ReportItem {
                code: w.code.to_string(),
                page: w.page.clone(),
                line: w.line,
                message: w.message.clone(),
            })
            .collect();
        for u in unresolved {
            warnings.push(ReportItem {
                code: "W001".to_string(),
                page: Some(u.page.clone()),
                line: Some(u.line),
                message: format!("unresolved link `{}` ({})", u.target, u.kind.describe()),
            });
        }
        warnings.sort_by(|a, b| {
            (&a.page, a.line, &a.code, &a.message).cmp(&(&b.page, b.line, &b.code, &b.message))
        });
        Self {
            errors,
            warnings,
            stats,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("report serialises")
    }
}

/// Print one error: first message line, then page, line, then any further message lines.
pub fn format_error(item: &ReportItem) -> String {
    let mut lines = item.message.lines();
    let mut out = format!("error[{}]: {}\n", item.code, lines.next().unwrap_or(""));
    if let Some(page) = &item.page {
        out.push_str(&format!("  page: {page}\n"));
    }
    if let Some(line) = item.line {
        out.push_str(&format!("  line: {line}\n"));
    }
    for l in lines {
        out.push_str(&format!("{l}\n"));
    }
    out
}

pub fn format_warning(item: &ReportItem) -> String {
    let location = match (&item.page, item.line) {
        (Some(p), Some(l)) => format!("{p}:{l}: "),
        (Some(p), None) => format!("{p}: "),
        _ => String::new(),
    };
    format!("warning[{}]: {location}{}\n", item.code, item.message)
}

/// Unresolved links grouped by PDF (F8).
pub fn format_unresolved(unresolved: &[UnresolvedLink]) -> String {
    let grouped: BTreeMap<_, _> = group_unresolved(unresolved);
    let mut out = String::new();
    for (pdf, links) in grouped {
        out.push_str(&format!(
            "Unresolved links in {pdf}.pdf ({}):\n",
            links.len()
        ));
        for l in links {
            out.push_str(&format!(
                "  {}:{}: `{}` ({})\n",
                l.page,
                l.line,
                l.target,
                l.kind.describe()
            ));
        }
    }
    out
}

/// Text report to `out` (stderr in the CLI). `quiet` hides warnings but never errors.
pub fn write_text(
    out: &mut dyn Write,
    report: &Report,
    unresolved: &[UnresolvedLink],
    quiet: bool,
) -> std::io::Result<()> {
    for e in &report.errors {
        write!(out, "{}", format_error(e))?;
    }
    if !quiet {
        for w in &report.warnings {
            write!(out, "{}", format_warning(w))?;
        }
        write!(out, "{}", format_unresolved(unresolved))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, HideoError, Warning, WarningCode};
    use crate::linkmap::UnresolvedKind;

    fn diag() -> DiagnosticCollector {
        let mut d = DiagnosticCollector::default();
        d.add_error(
            HideoError::new(ErrorCode::E004, "diagram failed to render\n  first line: x")
                .with_location("a.md", 4),
        );
        d.add_warning(Warning::new(WarningCode::W003, "hero missing").with_page("b.md"));
        d
    }

    fn link() -> UnresolvedLink {
        UnresolvedLink {
            pdf: "req".into(),
            page: "req/a.md".into(),
            line: 3,
            target: "gone.md".into(),
            kind: UnresolvedKind::MissingPage,
        }
    }

    #[test]
    fn test_check_mode_json_format() {
        let r = Report::new(
            &diag(),
            &[link()],
            Stats {
                tiers: 1,
                pages: 2,
                ..Default::default()
            },
        );
        let v: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
        assert_eq!(v["errors"][0]["code"], "E004");
        assert_eq!(v["errors"][0]["page"], "a.md");
        assert_eq!(v["errors"][0]["line"], 4);
        assert!(
            v["errors"][0]["message"]
                .as_str()
                .unwrap()
                .starts_with("diagram failed")
        );
        assert_eq!(v["warnings"].as_array().unwrap().len(), 2);
        assert!(
            v["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["code"] == "W001" && w["line"] == 3)
        );
        assert_eq!(v["stats"]["pages"], 2);
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, ["errors", "stats", "warnings"]);
    }

    #[test]
    fn error_format_follows_plan_f15() {
        let r = Report::new(&diag(), &[], Stats::default());
        assert_eq!(
            format_error(&r.errors[0]),
            "error[E004]: diagram failed to render\n  page: a.md\n  line: 4\n  first line: x\n"
        );
    }

    #[test]
    fn warnings_are_one_line_and_quiet_hides_them() {
        let r = Report::new(&diag(), &[link()], Stats::default());
        assert_eq!(
            format_warning(&r.warnings[0]),
            "warning[W003]: b.md: hero missing\n"
        );
        let mut loud = Vec::new();
        write_text(&mut loud, &r, &[link()], false).unwrap();
        let loud = String::from_utf8(loud).unwrap();
        assert!(loud.contains("Unresolved links in req.pdf (1):"));
        assert!(loud.contains("warning[W003]"));
        let mut quiet = Vec::new();
        write_text(&mut quiet, &r, &[link()], true).unwrap();
        let quiet = String::from_utf8(quiet).unwrap();
        assert!(
            quiet.contains("error[E004]")
                && !quiet.contains("warning[")
                && !quiet.contains("Unresolved")
        );
    }
}
