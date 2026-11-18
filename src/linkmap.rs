use std::collections::HashMap;
use camino::{Utf8PathBuf, Utf8Path};
use percent_encoding::percent_decode_str;
use crate::page::{Page, Heading};
use crate::error::{DiagnosticCollector, Warning, WarningCode, HideoError, ErrorCode};

#[derive(Debug, Clone)]
pub struct LinkMap {
    pub labels: HashMap<(String, String), String>,
    pub page_keys: HashMap<String, String>,
    pub lower_page_keys: HashMap<String, String>,
    pub unresolved: Vec<UnresolvedLink>,
}

#[derive(Debug, Clone)]
pub struct UnresolvedLink {
    pub page: String,
    pub line: usize,
    pub target: String,
    pub kind: UnresolvedKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedKind {
    MissingPage,
    MissingAnchor,
    OutsideTier,
    Directory,
    NonMarkdownFile,
}

impl LinkMap {
    pub fn new(pages: &[Page]) -> Self {
        let mut labels = HashMap::new();
        let mut page_keys = HashMap::new();
        let mut lower_page_keys = HashMap::new();

        for page in pages {
            let page_key = page.key.clone();
            page_keys.insert(page_key.clone(), page_key.clone());
            lower_page_keys.insert(page_key.to_lowercase(), page_key.clone());

            labels.insert((page_key.clone(), page_key.clone()), format!("<{}>", page_key));

            for heading in &page.headings {
                let label = format!("<{}--{}>", page_key, heading.id);
                labels.insert((page_key.clone(), heading.id.clone()), label);
            }
        }

        Self {
            labels,
            page_keys,
            lower_page_keys,
            unresolved: Vec::new(),
        }
    }

    pub fn resolve_link(
        &mut self,
        current_page: &Page,
        target: &str,
        line: usize,
    ) -> Option<String> {
        if is_external(target) {
            return Some(format!("#link(\"{}\")", escape_typst_string(target)));
        }

        let (path_part, anchor) = split_fragment(target);
        let decoded_path = percent_decode_str(path_part).decode_utf8_lossy().to_string();

        let current_dir = current_page.path.parent().unwrap_or(Utf8Path::new(""));
        let resolved_path = resolve_relative_path(current_dir, &decoded_path);

        let normalized = normalize_path(&resolved_path);

        if let Some(page_key) = self.page_keys.get(&normalized) {
            let label = if let Some(anchor) = anchor {
                let anchor_slug = generate_anchor_slug(anchor);
                self.labels.get(&(page_key.clone(), anchor_slug))
                    .cloned()
                    .unwrap_or_else(|| {
                        self.unresolved.push(UnresolvedLink {
                            page: current_page.key.clone(),
                            line,
                            target: target.to_string(),
                            kind: UnresolvedKind::MissingAnchor,
                        });
                        format!("<{}>", page_key)
                    })
            } else {
                format!("<{}>", page_key)
            };
            return Some(format!("#link({})", label));
        }

        if let Some(page_key) = self.lower_page_keys.get(&normalized.to_lowercase()) {
            self.unresolved.push(UnresolvedLink {
                page: current_page.key.clone(),
                line,
                target: target.to_string(),
                kind: UnresolvedKind::MissingPage,
            });
            let label = if let Some(anchor) = anchor {
                let anchor_slug = generate_anchor_slug(anchor);
                self.labels.get(&(page_key.clone(), anchor_slug))
                    .cloned()
                    .unwrap_or_else(|| format!("<{}>", page_key))
            } else {
                format!("<{}>", page_key)
            };
            return Some(format!("#link({})", label));
        }

        if is_directory(&normalized) || is_non_markdown_file(&normalized) {
            self.unresolved.push(UnresolvedLink {
                page: current_page.key.clone(),
                line,
                target: target.to_string(),
                kind: if is_directory(&normalized) {
                    UnresolvedKind::Directory
                } else {
                    UnresolvedKind::NonMarkdownFile
                },
            });
            return Some(format!("#link(\"{}\")", escape_typst_string(target)));
        }

        self.unresolved.push(UnresolvedLink {
            page: current_page.key.clone(),
            line,
            target: target.to_string(),
            kind: UnresolvedKind::MissingPage,
        });
        None
    }
}

fn is_external(target: &str) -> bool {
    if target.starts_with("mailto:") {
        return true;
    }
    if target.starts_with("//") {
        return true;
    }
    if let Some(colon_pos) = target.find(':') {
        let scheme = &target[..colon_pos];
        if scheme.len() >= 2 && scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
            return true;
        }
    }
    false
}

fn split_fragment(target: &str) -> (&str, Option<&str>) {
    if let Some(hash_pos) = target.find('#') {
        if hash_pos == 0 {
            ("", Some(&target[1..]))
        } else {
            (&target[..hash_pos], Some(&target[hash_pos + 1..]))
        }
    } else {
        (target, None)
    }
}

fn resolve_relative_path(base: &Utf8Path, relative: &str) -> Utf8PathBuf {
    let mut path = base.join(relative);
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            camino::Utf8Component::CurDir => {}
            camino::Utf8Component::ParentDir => {
                components.pop();
            }
            camino::Utf8Component::Normal(s) => {
                components.push(s.to_string());
            }
            _ => {}
        }
    }

    let mut result = Utf8PathBuf::new();
    for c in components {
        result.push(c);
    }
    result
}

fn normalize_path(path: &Utf8PathBuf) -> String {
    path.as_str().to_string()
}

fn is_directory(path: &str) -> bool {
    path.ends_with('/')
}

fn is_non_markdown_file(path: &str) -> bool {
    if let Some(ext) = path.split('.').last() {
        ext != "md"
    } else {
        false
    }
}

fn generate_anchor_slug(anchor: &str) -> String {
    let mut slug = String::new();
    let mut prev_was_hyphen = false;

    for c in anchor.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            slug.push(c.to_ascii_lowercase());
            prev_was_hyphen = false;
        } else if c.is_whitespace() {
            if !prev_was_hyphen && !slug.is_empty() {
                slug.push('-');
                prev_was_hyphen = true;
            }
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn escape_typst_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}