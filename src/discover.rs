//! Page discovery and ordering (plan F1-F3, 7.2 step 1).

use std::cmp::Ordering;
use std::path::Path;

use alphanumeric_sort::compare_str;
use camino::{Utf8Path, Utf8PathBuf};
use walkdir::WalkDir;

use crate::config::Config;
use crate::error::{DiagnosticCollector, ErrorCode, HideoError, Warning, WarningCode};
use crate::frontmatter::{self, Frontmatter};

/// A Markdown page read from disk, frontmatter already split off.
#[derive(Debug, Clone)]
pub struct SourcePage {
    pub tier: String,
    /// Path relative to the source folder with forward slashes, e.g. `required/01_intro.md`.
    /// This is the name used in diagnostics.
    pub rel_path: String,
    /// Absolute or CWD-relative path on disk.
    pub abs_path: Utf8PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct TierSources {
    pub tier: String,
    pub pages: Vec<SourcePage>,
}

/// Convert an OS path to UTF-8 or report E001 (the whole pipeline assumes UTF-8 names).
pub fn utf8_path(path: &Path) -> Result<Utf8PathBuf, HideoError> {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).map_err(|p| {
        HideoError::new(
            ErrorCode::E001,
            format!("file name is not valid UTF-8: {}", p.to_string_lossy()),
        )
        .with_page(p.to_string_lossy().into_owned())
    })
}

/// Forward-slash form of a relative path (stable across operating systems).
pub fn to_slash(path: &Utf8Path) -> String {
    path.as_str().replace('\\', "/")
}

/// Case-insensitive natural comparison of two slash separated paths, component by component,
/// so `Chapter 2/a.md` sorts before `chapter 10/a.md`. Ties fall back to the raw string so the
/// order is total and identical on every platform.
pub fn compare_paths(a: &str, b: &str) -> Ordering {
    let mut ia = a.split('/');
    let mut ib = b.split('/');
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = compare_str(x.to_lowercase(), y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

fn is_ignored(rel: &str, prefixes: &[String]) -> bool {
    rel.split('/').any(|component| {
        prefixes
            .iter()
            .any(|p| !p.is_empty() && component.starts_with(p.as_str()))
    })
}

/// Discover pages of all tiers named in `tiers`.
pub fn discover(
    config: &Config,
    tiers: &[String],
    source_dir: &Utf8Path,
) -> (Vec<TierSources>, DiagnosticCollector) {
    let mut diagnostics = DiagnosticCollector::default();
    let mut result = Vec::new();

    for tier in tiers {
        let tier_dir = source_dir.join(tier);
        if !tier_dir.is_dir() {
            diagnostics.add_warning(
                Warning::new(
                    WarningCode::W010,
                    format!("tier folder `{tier}` does not exist; skipped"),
                )
                .with_page(tier.clone()),
            );
            continue;
        }

        let mut files: Vec<(String, Utf8PathBuf)> = Vec::new();
        let walker = WalkDir::new(&tier_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Prune ignored directories early; the root itself is never pruned.
                if e.depth() == 0 || !e.file_type().is_dir() {
                    return true;
                }
                e.file_name()
                    .to_str()
                    .map(|n| {
                        !config
                            .ignore_prefixes
                            .iter()
                            .any(|p| !p.is_empty() && n.starts_with(p.as_str()))
                    })
                    .unwrap_or(true) // non-UTF-8 names are reported below
            });

        for entry in walker.filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let utf8 = match utf8_path(entry.path()) {
                Ok(p) => p,
                Err(e) => {
                    // Only report files that would have been pages.
                    if entry.path().extension().map(|x| x == "md").unwrap_or(false) {
                        diagnostics.add_error(e);
                    }
                    continue;
                }
            };
            if utf8.extension() != Some("md") {
                continue;
            }
            let rel = to_slash(utf8.strip_prefix(source_dir).unwrap_or(&utf8));
            let in_tier = rel
                .strip_prefix(&format!("{tier}/"))
                .unwrap_or(&rel)
                .to_string();
            if is_ignored(&in_tier, &config.ignore_prefixes) {
                continue;
            }
            files.push((rel, utf8));
        }

        files.sort_by(|a, b| compare_paths(&a.0, &b.0));

        let mut pages = Vec::new();
        for (rel, abs) in files {
            let raw = match std::fs::read(&abs) {
                Ok(bytes) => bytes,
                Err(e) => {
                    diagnostics.add_error(
                        HideoError::new(ErrorCode::E001, format!("cannot read page: {e}"))
                            .with_page(rel),
                    );
                    continue;
                }
            };
            let text = match String::from_utf8(raw) {
                Ok(t) => t,
                Err(_) => {
                    diagnostics.add_error(
                        HideoError::new(ErrorCode::E001, "page is not valid UTF-8").with_page(rel),
                    );
                    continue;
                }
            };
            match frontmatter::parse(&text) {
                Ok((fm, body)) => {
                    let missing = fm.validate(&rel);
                    if !missing.is_empty() {
                        for e in missing {
                            diagnostics.add_error(e);
                        }
                        continue;
                    }
                    pages.push(SourcePage {
                        tier: tier.clone(),
                        rel_path: rel,
                        abs_path: abs,
                        frontmatter: fm,
                        body,
                    });
                }
                Err(e) => diagnostics.add_error(
                    HideoError::new(ErrorCode::E001, e.message).with_location(rel, e.line),
                ),
            }
        }

        if pages.is_empty() && !diagnostics.has_errors() {
            diagnostics.add_warning(
                Warning::new(
                    WarningCode::W010,
                    format!("tier `{tier}` has no pages; skipped"),
                )
                .with_page(tier.clone()),
            );
            continue;
        }
        result.push(TierSources {
            tier: tier.clone(),
            pages,
        });
    }

    (result, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn config(tiers: &[&str]) -> Config {
        let list = tiers
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        Config::from_toml(
            &format!("tiers=[{list}]\n[book]\nlang=\"en\"\n[cover]\ntitle=\"T\"\n"),
            Utf8Path::new("."),
        )
        .unwrap()
    }

    fn page(title: &str) -> String {
        format!("---\ntitle: {title}\nhero: h.png\n---\nbody\n")
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn rels(t: &TierSources) -> Vec<&str> {
        t.pages.iter().map(|p| p.rel_path.as_str()).collect()
    }

    #[test]
    fn test_tiers_from_config() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a/x.md", &page("x"));
        write(dir.path(), "b/y.md", &page("y"));
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (tiers, d) = discover(&config(&["a", "b"]), &["a".into(), "b".into()], &root);
        assert!(!d.has_errors());
        assert_eq!(
            tiers.iter().map(|t| t.tier.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    #[test]
    fn test_empty_tier_skipped_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("empty")).unwrap();
        write(dir.path(), "full/x.md", &page("x"));
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (tiers, d) = discover(
            &config(&["empty", "full", "missing"]),
            &["empty".into(), "full".into(), "missing".into()],
            &root,
        );
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].tier, "full");
        assert_eq!(d.warnings.len(), 2, "{:?}", d.warnings);
        assert!(!d.has_errors());
    }

    #[test]
    fn test_ignore_prefixes_at_depth() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t/keep.md", &page("k"));
        write(dir.path(), "t/_draft.md", &page("d"));
        write(dir.path(), "t/.hidden.md", &page("h"));
        write(dir.path(), "t/sub/_wip/deep.md", &page("w"));
        write(dir.path(), "t/sub/ok.md", &page("o"));
        write(dir.path(), "t/sub/notes.txt", "x");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (tiers, _) = discover(&config(&["t"]), &["t".into()], &root);
        assert_eq!(rels(&tiers[0]), ["t/keep.md", "t/sub/ok.md"]);
    }

    #[test]
    fn test_page_order_case_insensitive_natural() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "10_ten.md",
            "2_two.md",
            "B_upper.md",
            "a_lower.md",
            "Chapter 2/x.md",
            "chapter 10/x.md",
        ] {
            write(dir.path(), &format!("t/{name}"), &page("p"));
        }
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (tiers, _) = discover(&config(&["t"]), &["t".into()], &root);
        assert_eq!(
            rels(&tiers[0]),
            [
                "t/2_two.md",
                "t/10_ten.md",
                "t/a_lower.md",
                "t/B_upper.md",
                "t/Chapter 2/x.md",
                "t/chapter 10/x.md"
            ]
        );
    }

    #[test]
    fn test_compare_paths_is_total_and_stable() {
        assert_eq!(compare_paths("t/A.md", "t/a.md"), "t/A.md".cmp("t/a.md"));
        assert_eq!(compare_paths("t/a", "t/a/b"), Ordering::Less);
        assert_eq!(compare_paths("t/a/b", "t/a"), Ordering::Greater);
    }

    #[cfg(unix)]
    #[test]
    fn test_non_utf8_path_rejected() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(OsStr::from_bytes(b"t/bad\xff.md"));
        let e = utf8_path(path).unwrap_err();
        assert_eq!(e.code, ErrorCode::E001);
    }

    #[test]
    fn test_invalid_utf8_file_content_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t/valid.md", &page("ok"));
        let bad_path = dir.path().join("t/bad_content.md");
        fs::write(bad_path, [0xff, 0xfe, 0xfd]).unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (_tiers, d) = discover(&config(&["t"]), &["t".into()], &root);
        assert!(d.errors.iter().any(|e| e.message.contains("valid UTF-8")));
    }

    #[test]
    fn test_frontmatter_problems_reported_with_page() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t/ok.md", &page("ok"));
        write(dir.path(), "t/notitle.md", "---\nhero: h\n---\nx");
        write(dir.path(), "t/unclosed.md", "---\ntitle: T\n");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (tiers, d) = discover(&config(&["t"]), &["t".into()], &root);
        assert_eq!(rels(&tiers[0]), ["t/ok.md"]);
        assert_eq!(d.errors.len(), 2);
        assert!(
            d.errors
                .iter()
                .any(|e| e.page.as_deref() == Some("t/notitle.md") && e.line == Some(1))
        );
        assert!(
            d.errors
                .iter()
                .any(|e| e.page.as_deref() == Some("t/unclosed.md") && e.line == Some(1))
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_unreadable_page_reported() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "t/unreadable.md", "content");
        let p = dir.path().join("t/unreadable.md");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o000)).unwrap();
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let (_tiers, d) = discover(&config(&["t"]), &["t".into()], &root);
        assert!(
            d.errors
                .iter()
                .any(|e| e.message.contains("cannot read page"))
        );
    }
}
