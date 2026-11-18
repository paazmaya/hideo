use std::collections::HashMap;
use camino::Utf8PathBuf;
use walkdir::WalkDir;
use alphanumeric_sort::compare_str;

use crate::config::Config;
use crate::error::{DiagnosticCollector, HideoError, ErrorCode, Warning, WarningCode};
use crate::frontmatter::Frontmatter;

#[derive(Debug, Clone)]
pub struct DiscoveredPage {
    pub path: Utf8PathBuf,
    pub tier: String,
    pub frontmatter: Frontmatter,
}

pub fn discover_pages(
    config: &Config,
    source_dir: &Utf8PathBuf,
    diagnostics: &mut DiagnosticCollector,
) -> Vec<DiscoveredPage> {
    let mut pages = Vec::new();

    for tier in &config.tiers {
        let tier_dir = source_dir.join(tier);
        if !tier_dir.exists() {
            diagnostics.add_warning(
                Warning::new(WarningCode::W001, format!("tier folder '{}' does not exist", tier))
                    .with_page(tier.clone()),
            );
            continue;
        }

        let mut tier_pages = Vec::new();

        for entry in WalkDir::new(&tier_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let utf8_path = match Utf8PathBuf::try_from(path.to_path_buf()) {
                Ok(p) => p,
                Err(_) => {
                    diagnostics.add_error(
                        HideoError::new(ErrorCode::E001, "non-UTF-8 file path")
                            .with_page(tier.clone()),
                    );
                    continue;
                }
            };

            if let Some(ext) = utf8_path.extension() {
                if ext != "md" {
                    continue;
                }
            } else {
                continue;
            }

            let file_name = utf8_path.file_name().unwrap_or("");
            if config.ignore_prefixes.iter().any(|p| file_name.starts_with(p)) {
                continue;
            }

            tier_pages.push(utf8_path);
        }

        tier_pages.sort_by(|a, b| {
            let a_rel = a.strip_prefix(&tier_dir).unwrap_or(a);
            let b_rel = b.strip_prefix(&tier_dir).unwrap_or(b);
            compare_str(a_rel.as_str(), b_rel.as_str())
        });

        for page_path in tier_pages {
            let content = match std::fs::read_to_string(&page_path) {
                Ok(c) => c,
                Err(e) => {
                    diagnostics.add_error(
                        HideoError::new(ErrorCode::E001, format!("failed to read page: {e}"))
                            .with_page(tier.clone()),
                    );
                    continue;
                }
            };

            let (frontmatter, body) = match crate::frontmatter::parse(&content) {
                Ok((fm, body)) => (fm, body),
                Err(e) => {
                    diagnostics.add_error(
                        HideoError::new(ErrorCode::E001, e.to_string())
                            .with_page(tier.clone())
                            .with_line(e.line.unwrap_or(1)),
                    );
                    continue;
                }
            };

            if frontmatter.title.is_none() {
                diagnostics.add_error(
                    HideoError::new(ErrorCode::E001, "missing required frontmatter field: title")
                        .with_page(tier.clone())
                        .with_line(1),
                );
            }

            if frontmatter.hero.is_none() {
                diagnostics.add_warning(
                    Warning::new(WarningCode::W003, "missing hero image, will use placeholder")
                        .with_page(tier.clone()),
                );
            }

            let rel_path = page_path.strip_prefix(&tier_dir).unwrap_or(&page_path).to_owned();
            pages.push(DiscoveredPage {
                path: rel_path,
                tier: tier.clone(),
                frontmatter,
            });
        }
    }

    pages
}