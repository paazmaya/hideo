use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::error::{DiagnosticCollector, HideoError, Warning};
use crate::linkmap::UnresolvedLink;

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub errors: Vec<ReportItem>,
    pub warnings: Vec<ReportItem>,
    pub stats: Stats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReportItem {
    pub code: String,
    pub page: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stats {
    pub pages: usize,
    pub images: usize,
    pub diagrams: usize,
    pub tiers: usize,
}

pub fn generate_report(diagnostics: &DiagnosticCollector, unresolved: &[UnresolvedLink], stats: Stats) -> Report {
    let mut errors: Vec<ReportItem> = diagnostics.errors.iter().map(|e| ReportItem {
        code: format!("{:?}", e.code),
        page: e.page.clone(),
        line: e.line,
        message: e.message.clone(),
    }).collect();

    for u in unresolved {
        errors.push(ReportItem {
            code: format!("{:?}", u.kind),
            page: Some(u.page.clone()),
            line: Some(u.line),
            message: format!("unresolved link: {}", u.target),
        });
    }

    let warnings: Vec<ReportItem> = diagnostics.warnings.iter().map(|w| ReportItem {
        code: format!("{:?}", w.code),
        page: w.page.clone(),
        line: w.line,
        message: w.message.clone(),
    }).collect();

    Report { errors, warnings, stats }
}

pub fn print_report(report: &Report, format: &str) {
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(report).unwrap());
        }
        _ => {
            if !report.errors.is_empty() {
                eprintln!("Errors:");
                for e in &report.errors {
                    let loc = match (&e.page, &e.line) {
                        (Some(p), Some(l)) => format!("{}:{}", p, l),
                        (Some(p), None) => p.clone(),
                        _ => "unknown".to_string(),
                    };
                    eprintln!("  [{}] {}: {}", e.code, loc, e.message);
                }
            }

            if !report.warnings.is_empty() {
                eprintln!("Warnings:");
                for w in &report.warnings {
                    let loc = match (&w.page, &w.line) {
                        (Some(p), Some(l)) => format!("{}:{}", p, l),
                        (Some(p), None) => p.clone(),
                        _ => "unknown".to_string(),
                    };
                    eprintln!("  [{}] {}: {}", w.code, loc, w.message);
                }
            }

            eprintln!("Stats: {} pages, {} images, {} diagrams, {} tiers",
                report.stats.pages, report.stats.images, report.stats.diagrams, report.stats.tiers);
        }
    }
}