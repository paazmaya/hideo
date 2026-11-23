//! Labels, link resolution and the unresolved-link report (plan F5-F8).
//!
//! Typst labels are global to a document, so every label is qualified by the page key:
//! the page itself is `<page_key>` and a heading is `<page_key--heading_id>`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use percent_encoding::percent_decode_str;

use crate::error::{Warning, WarningCode};
use crate::page::{Page, slugify};

/// How a link destination was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// Another page (or heading) of the same PDF: a Typst label without the angle brackets.
    Internal { label: String },
    /// Leaves the PDF to the web or a mail client. `arrow` is false for `mailto:`.
    External { url: String, arrow: bool },
    /// A file or directory next to the sources: kept as an ordinary link to the path.
    File { path: String },
    /// Could not be linked; rendered as plain text. The reason is in the report.
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnresolvedKind {
    MissingPage,
    MissingAnchor,
    OutsidePdf,
    MissingFile,
}

impl UnresolvedKind {
    pub fn describe(self) -> &'static str {
        match self {
            Self::MissingPage => "page not found",
            Self::MissingAnchor => "heading not found",
            Self::OutsidePdf => "outside this PDF",
            Self::MissingFile => "file not found",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnresolvedLink {
    /// Tier, i.e. the PDF the link is in.
    pub pdf: String,
    pub page: String,
    pub line: usize,
    pub target: String,
    pub kind: UnresolvedKind,
}

/// Link resolution for one tier (= one PDF).
pub struct LinkMap {
    tier: String,
    source_dir: std::path::PathBuf,
    /// Source-relative page path -> page key.
    pages: HashMap<String, String>,
    /// Lower-cased page path -> exact page path, for case-mismatch warnings.
    lower_pages: HashMap<String, String>,
    /// Page key -> heading ids.
    headings: HashMap<String, HashSet<String>>,
    /// Pages of all other tiers, to recognise links that leave the PDF.
    other_tier_pages: HashSet<String>,
    pub unresolved: Vec<UnresolvedLink>,
    pub warnings: Vec<Warning>,
}

impl LinkMap {
    pub fn new(
        tier: &str,
        pages: &[Page],
        other_tier_pages: impl IntoIterator<Item = String>,
        source_dir: &Path,
    ) -> Self {
        let mut map = HashMap::new();
        let mut lower = HashMap::new();
        let mut headings = HashMap::new();
        for p in pages {
            map.insert(p.rel_path.clone(), p.key.clone());
            lower
                .entry(p.rel_path.to_lowercase())
                .or_insert_with(|| p.rel_path.clone());
            headings.insert(
                p.key.clone(),
                p.headings.iter().map(|h| h.id.clone()).collect(),
            );
        }
        Self {
            tier: tier.to_string(),
            source_dir: source_dir.to_path_buf(),
            pages: map,
            lower_pages: lower,
            headings,
            other_tier_pages: other_tier_pages.into_iter().collect(),
            unresolved: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Typst label for a heading.
    pub fn heading_label(page_key: &str, heading_id: &str) -> String {
        format!("{page_key}--{heading_id}")
    }

    /// Resolve `target` as written in `from` at file line `line`.
    pub fn resolve(&mut self, from: &Page, target: &str, line: usize) -> Resolved {
        if is_external(target) {
            return Resolved::External {
                url: target.to_string(),
                arrow: !target.to_ascii_lowercase().starts_with("mailto:"),
            };
        }

        let (path_part, fragment) = match target.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (target, None),
        };
        let decoded_path = percent_decode_str(path_part)
            .decode_utf8_lossy()
            .into_owned();
        let decoded_fragment =
            fragment.map(|f| percent_decode_str(f).decode_utf8_lossy().into_owned());

        // Fragment-only: the current page.
        let (page_path, page_key) = if decoded_path.is_empty() {
            (from.rel_path.clone(), from.key.clone())
        } else {
            let dir = from.rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let resolved = normalize_lexically(dir, &decoded_path);
            match self.lookup_page(&resolved) {
                Lookup::Found {
                    path,
                    key,
                    case_mismatch,
                } => {
                    if case_mismatch {
                        self.warnings.push(
                            Warning::new(
                                WarningCode::W005,
                                format!("link `{target}` differs in case from the file on disk (`{path}`)"),
                            )
                            .with_location(from.rel_path.clone(), line),
                        );
                    }
                    (path, key)
                }
                Lookup::OtherTier => {
                    self.report(from, line, target, UnresolvedKind::OutsidePdf);
                    return Resolved::Unresolved;
                }
                Lookup::None => return self.resolve_file(from, line, target, &resolved),
            }
        };

        match decoded_fragment.as_deref().filter(|f| !f.is_empty()) {
            None => Resolved::Internal { label: page_key },
            Some(fragment) => {
                let ids = self.headings.get(&page_key);
                let candidates = [fragment.to_string(), slugify(fragment)];
                match candidates
                    .iter()
                    .find(|c| ids.is_some_and(|ids| ids.contains(c.as_str())))
                {
                    Some(id) => Resolved::Internal {
                        label: Self::heading_label(&page_key, id),
                    },
                    None => {
                        self.report(from, line, target, UnresolvedKind::MissingAnchor);
                        // The page itself still exists, so link to it.
                        let _ = page_path;
                        Resolved::Internal { label: page_key }
                    }
                }
            }
        }
    }

    fn lookup_page(&self, resolved: &str) -> Lookup {
        if let Some(key) = self.pages.get(resolved) {
            return Lookup::Found {
                path: resolved.to_string(),
                key: key.clone(),
                case_mismatch: false,
            };
        }
        if let Some(exact) = self.lower_pages.get(&resolved.to_lowercase()) {
            let key = self.pages[exact].clone();
            return Lookup::Found {
                path: exact.clone(),
                key,
                case_mismatch: true,
            };
        }
        if self.other_tier_pages.contains(resolved) {
            return Lookup::OtherTier;
        }
        Lookup::None
    }

    /// Not a page of any tier. Non-Markdown files and directories stay ordinary links and are
    /// reported only if they do not exist; a missing `.md` is a missing page.
    fn resolve_file(&mut self, from: &Page, line: usize, target: &str, resolved: &str) -> Resolved {
        let is_markdown = resolved.ends_with(".md");
        let on_disk = self.source_dir.join(resolved);
        if !is_markdown && (on_disk.is_file() || on_disk.is_dir()) {
            return Resolved::File {
                path: target.to_string(),
            };
        }
        let kind = if is_markdown {
            UnresolvedKind::MissingPage
        } else {
            UnresolvedKind::MissingFile
        };
        self.report(from, line, target, kind);
        Resolved::Unresolved
    }

    fn report(&mut self, from: &Page, line: usize, target: &str, kind: UnresolvedKind) {
        self.unresolved.push(UnresolvedLink {
            pdf: self.tier.clone(),
            page: from.rel_path.clone(),
            line,
            target: target.to_string(),
            kind,
        });
    }
}

enum Lookup {
    Found {
        path: String,
        key: String,
        case_mismatch: bool,
    },
    OtherTier,
    None,
}

/// A destination is external when it starts with `scheme:` (two or more scheme characters, so
/// `C:\x` is not a scheme) or `//`.
pub fn is_external(target: &str) -> bool {
    if target.starts_with("//") {
        return true;
    }
    let Some(colon) = target.find(':') else {
        return false;
    };
    let scheme = &target[..colon];
    scheme.len() >= 2
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

/// Join `relative` onto `base_dir` and normalise `.` and `..` without touching the filesystem
/// (`canonicalize` fails on missing files and yields `\\?\` paths on Windows). Backslashes in
/// the link are treated as separators. A `..` that climbs above the root is kept so the result
/// cannot accidentally match a page.
pub fn normalize_lexically(base_dir: &str, relative: &str) -> String {
    let relative = relative.replace('\\', "/");
    let mut parts: Vec<&str> = if relative.starts_with('/') {
        Vec::new()
    } else {
        base_dir.split('/').filter(|p| !p.is_empty()).collect()
    };
    for component in relative.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if matches!(parts.last(), Some(last) if *last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            c => parts.push(c),
        }
    }
    parts.join("/")
}

/// Unresolved links grouped by PDF and sorted (F8).
pub fn group_unresolved(links: &[UnresolvedLink]) -> BTreeMap<String, Vec<UnresolvedLink>> {
    let mut grouped: BTreeMap<String, Vec<UnresolvedLink>> = BTreeMap::new();
    for l in links {
        grouped.entry(l.pdf.clone()).or_default().push(l.clone());
    }
    for v in grouped.values_mut() {
        v.sort();
        v.dedup();
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::SourcePage;
    use crate::frontmatter::Frontmatter;
    use camino::Utf8PathBuf;

    fn page(rel: &str, key: &str, body: &str) -> Page {
        let src = SourcePage {
            tier: rel.split('/').next().unwrap().to_string(),
            rel_path: rel.to_string(),
            abs_path: Utf8PathBuf::from(rel),
            frontmatter: Frontmatter {
                body_line_offset: 4,
                ..Default::default()
            },
            body: body.to_string(),
        };
        Page::build(src, key.to_string()).unwrap()
    }

    fn fixture() -> (LinkMap, Page, Page) {
        let a = page("req/a.md", "req-a", "## Setup\n\n## Other {#custom}\n");
        let b = page("req/sub/B File.md", "req-sub-b-file", "## Intro\n");
        let map = LinkMap::new(
            "req",
            &[a.clone(), b.clone()],
            ["add/x.md".to_string()],
            Path::new("definitely/not/here"),
        );
        (map, a, b)
    }

    #[test]
    fn test_link_external_unchanged() {
        let (mut m, a, _) = fixture();
        assert_eq!(
            m.resolve(&a, "https://example.com/x", 5),
            Resolved::External {
                url: "https://example.com/x".into(),
                arrow: true
            }
        );
        assert_eq!(
            m.resolve(&a, "mailto:a@b.c", 5),
            Resolved::External {
                url: "mailto:a@b.c".into(),
                arrow: false
            }
        );
        assert!(matches!(
            m.resolve(&a, "//cdn.example/x", 5),
            Resolved::External { .. }
        ));
        assert!(m.unresolved.is_empty());
    }

    #[test]
    fn test_link_scheme_vs_windows_drive() {
        assert!(is_external("https://x"));
        assert!(is_external("ftp://x"));
        assert!(is_external("mailto:a@b"));
        assert!(!is_external(r"C:\x\y.md"));
        assert!(!is_external("C:/x/y.md"));
        assert!(!is_external("a.md"));
        assert!(!is_external("a:b")); // one-letter "scheme"
        assert!(!is_external("./a.md#x:y"));
        assert!(!is_external(""));
        assert!(!is_external(":"));
        assert!(!is_external(":abc"));
        assert!(!is_external("123:abc"));
        assert!(!is_external("a$:abc"));
        assert!(is_external("//example.com/test"));
    }

    #[test]
    fn test_link_percent_encoded_path() {
        let (mut m, a, _) = fixture();
        assert_eq!(
            m.resolve(&a, "sub/B%20File.md", 5),
            Resolved::Internal {
                label: "req-sub-b-file".into()
            }
        );
        assert!(m.unresolved.is_empty());
    }

    #[test]
    fn test_link_non_md_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("req/files")).unwrap();
        std::fs::write(dir.path().join("req/files/data.zip"), b"x").unwrap();
        let a = page("req/a.md", "req-a", "");
        let mut m = LinkMap::new("req", std::slice::from_ref(&a), [], dir.path());
        assert_eq!(
            m.resolve(&a, "files/data.zip", 7),
            Resolved::File {
                path: "files/data.zip".into()
            }
        );
        assert_eq!(
            m.resolve(&a, "files/", 7),
            Resolved::File {
                path: "files/".into()
            }
        );
        assert!(m.unresolved.is_empty());
        assert_eq!(m.resolve(&a, "files/gone.zip", 8), Resolved::Unresolved);
        assert_eq!(m.unresolved[0].kind, UnresolvedKind::MissingFile);
    }

    #[test]
    fn test_link_same_page_anchor() {
        let (mut m, a, _) = fixture();
        assert_eq!(
            m.resolve(&a, "#setup", 5),
            Resolved::Internal {
                label: "req-a--setup".into()
            }
        );
        assert_eq!(
            m.resolve(&a, "#custom", 5),
            Resolved::Internal {
                label: "req-a--custom".into()
            }
        );
    }

    #[test]
    fn test_link_same_page_no_anchor_to_first_heading() {
        let (mut m, a, _) = fixture();
        // A bare `#` or `a.md` links to the page anchor, i.e. the top of the page.
        assert_eq!(
            m.resolve(&a, "a.md", 5),
            Resolved::Internal {
                label: "req-a".into()
            }
        );
        assert_eq!(
            m.resolve(&a, "#", 5),
            Resolved::Internal {
                label: "req-a".into()
            }
        );
    }

    #[test]
    fn test_link_cross_page() {
        let (mut m, a, b) = fixture();
        assert_eq!(
            m.resolve(&a, "sub/B File.md", 5),
            Resolved::Internal {
                label: "req-sub-b-file".into()
            }
        );
        assert_eq!(
            m.resolve(&b, "../a.md", 5),
            Resolved::Internal {
                label: "req-a".into()
            }
        );
    }

    #[test]
    fn test_link_cross_page_anchor() {
        let (mut m, a, b) = fixture();
        assert_eq!(
            m.resolve(&a, "sub/B%20File.md#intro", 5),
            Resolved::Internal {
                label: "req-sub-b-file--intro".into()
            }
        );
        assert_eq!(
            m.resolve(&b, "../a.md#Other-Setup", 5),
            Resolved::Internal {
                label: "req-a".into()
            }
        );
        assert_eq!(m.unresolved.len(), 1);
    }

    #[test]
    fn test_link_case_mismatch_resolves_with_warning() {
        let (mut m, a, _) = fixture();
        assert_eq!(
            m.resolve(&a, "SUB/b file.md", 9),
            Resolved::Internal {
                label: "req-sub-b-file".into()
            }
        );
        assert_eq!(m.warnings.len(), 1);
        assert_eq!(m.warnings[0].code, WarningCode::W005);
        assert_eq!(m.warnings[0].line, Some(9));
        assert!(m.unresolved.is_empty());
    }

    #[test]
    fn test_link_outside_pdf_marked() {
        let (mut m, a, _) = fixture();
        assert_eq!(m.resolve(&a, "../add/x.md", 5), Resolved::Unresolved);
        assert_eq!(m.unresolved[0].kind, UnresolvedKind::OutsidePdf);
        assert_eq!(m.unresolved[0].pdf, "req");
        assert_eq!(m.unresolved[0].page, "req/a.md");
    }

    #[test]
    fn test_link_missing_anchor_marked() {
        let (mut m, a, _) = fixture();
        // The page exists, so the link still works at page level.
        assert_eq!(
            m.resolve(&a, "#nope", 12),
            Resolved::Internal {
                label: "req-a".into()
            }
        );
        assert_eq!(m.unresolved[0].kind, UnresolvedKind::MissingAnchor);
        assert_eq!(m.unresolved[0].line, 12);
    }

    #[test]
    fn test_link_missing_page_marked() {
        let (mut m, a, _) = fixture();
        assert_eq!(m.resolve(&a, "gone.md", 3), Resolved::Unresolved);
        assert_eq!(m.unresolved[0].kind, UnresolvedKind::MissingPage);
    }

    #[test]
    fn test_link_directory_unresolved() {
        let (mut m, a, _) = fixture();
        // Directory that is not on disk.
        assert_eq!(m.resolve(&a, "nowhere/", 3), Resolved::Unresolved);
        assert_eq!(m.unresolved[0].kind, UnresolvedKind::MissingFile);
    }

    #[test]
    fn test_report_grouped_by_pdf_sorted() {
        let mk = |pdf: &str, page: &str, line| UnresolvedLink {
            pdf: pdf.into(),
            page: page.into(),
            line,
            target: "x.md".into(),
            kind: UnresolvedKind::MissingPage,
        };
        let links = [
            mk("b", "b/2.md", 3),
            mk("a", "a/9.md", 1),
            mk("b", "b/1.md", 9),
            mk("a", "a/9.md", 1),
        ];
        let grouped = group_unresolved(&links);
        assert_eq!(grouped.keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(grouped["a"].len(), 1, "duplicates collapse");
        assert_eq!(grouped["b"][0].page, "b/1.md");
    }

    #[test]
    fn test_normalize_lexically() {
        assert_eq!(normalize_lexically("t/sub", "../a.md"), "t/a.md");
        assert_eq!(normalize_lexically("t/sub", "./x/./y.md"), "t/sub/x/y.md");
        assert_eq!(normalize_lexically("t", "..\\a.md"), "a.md");
        assert_eq!(normalize_lexically("t", "../../a.md"), "../a.md");
        assert_eq!(normalize_lexically("", "a.md"), "a.md");
    }
}
