//! Diagram rendering (plan 7.7, F15).
//!
//! The backend is a command contract: an `mmdc`-compatible CLI (`mmdr` has the same flags).
//! Whatever the backend, [`check_svg`] enforces the post-conditions: the output is an SVG and it
//! contains no `<foreignObject>` (Typst renders those incorrectly, labels vanish).

pub mod external;
#[cfg(feature = "native-diagrams")]
pub mod native;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::config::DEFAULT_DIAGRAM_THEME;
use crate::error::{ErrorCode, HideoError, Warning, WarningCode};

/// Bump when diagram post-processing changes, to invalidate caches.
pub const DIAGRAM_PIPELINE_VERSION: &str = "hideo_diagram_v2";

/// Font families embedded in the binary (via `typst-kit/embedded-fonts`).
pub const EMBEDDED_FAMILIES: [&str; 5] = [
    "readex pro",
    "libertinus serif",
    "new computer modern",
    "new computer modern math",
    "dejavu sans mono",
];

#[derive(Debug, Clone)]
pub struct Theme {
    pub json: String,
}

impl Theme {
    pub fn embedded() -> Self {
        Self {
            json: DEFAULT_DIAGRAM_THEME.to_string(),
        }
    }

    pub fn from_file(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read diagram theme {}: {e}", path.display()))?;
        serde_json::from_str::<serde_json::Value>(&json)
            .map_err(|e| format!("diagram theme {} is not valid JSON: {e}", path.display()))?;
        Ok(Self { json })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DiagramError {
    #[error("{0}")]
    RenderFailed(String),
    #[error("the renderer did not finish within {0} seconds and was killed")]
    Timeout(u64),
    #[error(
        "the output contains <foreignObject>, which Typst renders incorrectly (labels vanish); set htmlLabels to false in the diagram theme"
    )]
    ForeignObject,
    #[error("the output is not an SVG document")]
    InvalidSvg,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub trait DiagramRenderer: Send + Sync {
    /// Render Mermaid source to an SVG string.
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError>;
    /// Human readable backend name for error messages, e.g. `external (mmdc)`.
    fn name(&self) -> String;
    /// Identity used in the cache key (name plus version information).
    fn cache_id(&self) -> String {
        self.name()
    }
}

/// Post-conditions for every backend (plan 7.7). Returns warnings about fonts.
pub fn check_svg(svg: &str) -> Result<Vec<String>, DiagramError> {
    let mut rest = svg.trim_start();
    // Skip an XML prolog, comments and a doctype.
    loop {
        if rest.starts_with("<?") {
            match rest.find("?>") {
                Some(i) => rest = rest[i + 2..].trim_start(),
                None => return Err(DiagramError::InvalidSvg),
            }
        } else if rest.starts_with("<!--") {
            match rest.find("-->") {
                Some(i) => rest = rest[i + 3..].trim_start(),
                None => return Err(DiagramError::InvalidSvg),
            }
        } else if rest.starts_with("<!DOCTYPE") || rest.starts_with("<!doctype") {
            match rest.find('>') {
                Some(i) => rest = rest[i + 1..].trim_start(),
                None => return Err(DiagramError::InvalidSvg),
            }
        } else {
            break;
        }
    }
    if !rest.starts_with("<svg") {
        return Err(DiagramError::InvalidSvg);
    }
    if svg.contains("<foreignObject") || svg.contains("<foreignobject") {
        return Err(DiagramError::ForeignObject);
    }
    Ok(font_problems(svg))
}

/// Replaces `font-family` attributes that hold only a generic family with an embedded one.
/// Mermaid hardcodes `font-family="sans-serif"` on the axis of gantt charts, outside the
/// theme's reach, and a generic name makes Typst pick whatever face it finds first.
pub fn normalize_fonts(svg: &str) -> String {
    const GENERIC: [(&str, &str); 3] = [
        (
            "font-family=\"sans-serif\"",
            "font-family=\"Readex Pro, sans-serif\"",
        ),
        (
            "font-family=\"serif\"",
            "font-family=\"Libertinus Serif, serif\"",
        ),
        (
            "font-family=\"monospace\"",
            "font-family=\"DejaVu Sans Mono, monospace\"",
        ),
    ];
    GENERIC
        .iter()
        .fold(svg.to_string(), |acc, (from, to)| acc.replace(from, to))
}

/// `font-family` declarations in the SVG that do not name an embedded font. Typst resolves SVG
/// text through its own font book, so anything else silently falls back to another face.
pub fn font_problems(svg: &str) -> Vec<String> {
    let mut problems = Vec::new();
    // mermaid writes `font-family: &quot;Libertinus Serif&quot;, ...` in CSS; the `;` of the
    // entity must not end the declaration.
    let unescaped = svg.replace("&quot;", "\"");
    let mut rest = unescaped.as_str();
    while let Some(i) = rest.find("font-family") {
        rest = &rest[i + "font-family".len()..];
        let after = rest.trim_start();
        let value = if let Some(attr) = after.strip_prefix('=') {
            // `font-family="A, B"`: up to the matching quote.
            let attr = attr.trim_start();
            match attr.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let inner = &attr[1..];
                    &inner[..inner.find(q).unwrap_or(inner.len())]
                }
                _ => continue,
            }
        } else if let Some(decl) = after.strip_prefix(':') {
            // CSS: `font-family: A, B;` up to `;`, `}` or the end of the tag.
            let end = decl.find([';', '}', '>', '\n']).unwrap_or(decl.len());
            &decl[..end]
        } else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        let families: Vec<String> = value
            .split(',')
            .map(|f| {
                f.trim()
                    .trim_matches(['"', '\'', '\\'])
                    .replace("&quot;", "")
                    .to_lowercase()
            })
            .collect();
        if !value.is_empty()
            && !families
                .iter()
                .any(|f| EMBEDDED_FAMILIES.contains(&f.as_str()))
        {
            let problem = value.to_string();
            if !problems.contains(&problem) {
                problems.push(problem);
            }
        }
    }
    problems
}

/// The first word of a Mermaid source: the diagram type keyword (`graph`, `sequenceDiagram`,
/// ...). Leading blank lines, `%%` comments and init directives, and a `---` frontmatter block
/// are skipped.
pub fn diagram_kind(source: &str) -> &str {
    let mut in_frontmatter = false;
    for line in source.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if line == "---" {
            if !in_frontmatter {
                in_frontmatter = true;
                continue;
            }
            in_frontmatter = false;
            continue;
        }
        if in_frontmatter {
            continue;
        }
        return line
            .split(|c: char| c.is_whitespace() || c == ';')
            .next()
            .unwrap_or("");
    }
    ""
}

/// Diagram types the native backend renders well enough for the book:
/// the other types go to the fallback renderer. Sequence diagrams (grey, off-theme participant
/// boxes) and class diagrams (members run together on one line) lost the side-by-side comparison
/// with `mmdc` 12.2 and stay with the external renderer.
pub const NATIVE_KINDS: &[&str] = &["graph", "flowchart", "stateDiagram", "stateDiagram-v2"];

/// Routes each diagram to `primary` when its type is in `kinds`, and to `fallback` otherwise.
pub struct RoutingRenderer {
    pub primary: Box<dyn DiagramRenderer>,
    pub fallback: Box<dyn DiagramRenderer>,
    pub kinds: Vec<String>,
}

impl RoutingRenderer {
    fn chooses_primary(&self, source: &str) -> bool {
        let kind = diagram_kind(source);
        self.kinds.iter().any(|k| k == kind)
    }
}

impl DiagramRenderer for RoutingRenderer {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError> {
        if self.chooses_primary(source) {
            self.primary.render(source, theme)
        } else {
            self.fallback.render(source, theme)
        }
    }

    fn name(&self) -> String {
        format!(
            "{} with fallback to {}",
            self.primary.name(),
            self.fallback.name()
        )
    }

    fn cache_id(&self) -> String {
        format!(
            "routing[{}]({}|{})",
            self.kinds.join(","),
            self.primary.cache_id(),
            self.fallback.cache_id()
        )
    }
}

/// Virtual path for a diagram source: `diagrams/<hash16>.svg`.
pub fn diagram_vpath(source: &str) -> String {
    format!(
        "diagrams/{}.svg",
        &blake3::hash(source.as_bytes()).to_hex()[..16]
    )
}

fn disk_cache_key(source: &str, theme: &Theme, renderer: &dyn DiagramRenderer) -> String {
    let mut h = blake3::Hasher::new();
    h.update(source.as_bytes());
    h.update(theme.json.as_bytes());
    h.update(renderer.cache_id().as_bytes());
    h.update(DIAGRAM_PIPELINE_VERSION.as_bytes());
    h.finalize().to_hex()[..32].to_string()
}

/// Render with the disk cache `<cache_dir>/diagrams/<hash>.svg`.
pub fn render_cached(
    renderer: &dyn DiagramRenderer,
    theme: &Theme,
    cache_dir: Option<&Path>,
    source: &str,
) -> Result<(String, Vec<String>), DiagramError> {
    let key = disk_cache_key(source, theme, renderer);
    let path: Option<PathBuf> = cache_dir.map(|d| d.join("diagrams").join(format!("{key}.svg")));
    if let Some(cached) = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok())
        && let Ok(fonts) = check_svg(&cached)
    {
        return Ok((cached, fonts));
    }
    let svg = normalize_fonts(&renderer.render(source, theme)?);
    let fonts = check_svg(&svg)?;
    if let Some(path) = path {
        let _ = write_cache(&path, svg.as_bytes());
    }
    Ok((svg, fonts))
}

fn write_cache(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().expect("cache path has a parent");
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// Where a diagram occurs in the Markdown.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Occurrence {
    pub page: String,
    pub line: usize,
}

/// Collects diagram requests while pages are emitted; rendering happens afterwards, in parallel
/// and deduplicated by source (plan 7.2 step 8).
#[derive(Debug, Default)]
pub struct DiagramCollector {
    /// vpath -> source
    pub sources: BTreeMap<String, String>,
    pub occurrences: BTreeMap<String, Vec<Occurrence>>,
}

impl DiagramCollector {
    pub fn request(&mut self, page: &str, line: usize, source: &str) -> String {
        let vpath = diagram_vpath(source);
        self.sources
            .entry(vpath.clone())
            .or_insert_with(|| source.to_string());
        self.occurrences
            .entry(vpath.clone())
            .or_default()
            .push(Occurrence {
                page: page.to_string(),
                line,
            });
        vpath
    }
}

#[derive(Debug, Default)]
pub struct RenderedDiagrams {
    pub svgs: BTreeMap<String, String>,
    pub errors: Vec<HideoError>,
    pub warnings: Vec<Warning>,
    /// Number of diagrams answered from the disk cache or rendered, for statistics.
    pub rendered: usize,
}

/// Render diagrams in source order, stopping after the first failure.
pub fn render_all(
    collector: &DiagramCollector,
    renderer: &dyn DiagramRenderer,
    theme: &Theme,
    cache_dir: Option<&Path>,
) -> RenderedDiagrams {
    let mut entries: Vec<(&String, &String)> = collector.sources.iter().collect();
    entries.sort_by_key(|(vpath, _)| {
        collector.occurrences[*vpath]
            .iter()
            .min()
            .map(|o| (o.page.as_str(), o.line))
            .unwrap_or(("", 0))
    });

    let mut out = RenderedDiagrams::default();
    for (vpath, source) in entries {
        let result = if source.trim().is_empty() {
            Err(DiagramError::RenderFailed(
                "the diagram is empty".to_string(),
            ))
        } else {
            render_cached(renderer, theme, cache_dir, source)
        };
        let occurrences = &collector.occurrences[vpath];
        match result {
            Ok((svg, fonts)) => {
                out.rendered += 1;
                for font in fonts {
                    for o in occurrences {
                        out.warnings.push(
                            Warning::new(
                                WarningCode::W010,
                                format!(
                                    "diagram text uses font-family `{font}`, which is not embedded; set the diagram theme's fontFamily to \"Readex Pro\""
                                ),
                            )
                            .with_location(o.page.clone(), o.line),
                        );
                    }
                }
                out.svgs.insert(vpath.clone(), svg);
            }
            Err(e) => {
                let occurrence = occurrences
                    .iter()
                    .min()
                    .expect("a collected diagram has an occurrence");
                out.errors.push(diagram_error(
                    &occurrence.page,
                    occurrence.line,
                    source,
                    &renderer.name(),
                    &e,
                ));
                break;
            }
        }
    }
    out
}

/// The error format of plan F15.
pub fn diagram_error(
    page: &str,
    line: usize,
    source: &str,
    backend: &str,
    reason: &DiagramError,
) -> HideoError {
    let first: String = source
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(60)
        .collect();
    let reason = reason.to_string();
    let reason = reason.trim().replace('\n', "\n    ");
    HideoError::new(
        ErrorCode::E004,
        format!("diagram failed to render\n  first line: {first}\n  backend: {backend}\n  reason: {reason}"),
    )
    .with_location(page.to_string(), line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagram_foreignobject_rejected() {
        let svg =
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><foreignObject>x</foreignObject></svg>";
        assert!(matches!(check_svg(svg), Err(DiagramError::ForeignObject)));
    }

    #[test]
    fn svg_post_conditions() {
        assert!(check_svg("<svg></svg>").is_ok());
        assert!(check_svg("<?xml version=\"1.0\"?>\n<!-- c -->\n<svg></svg>").is_ok());
        assert!(check_svg("<!DOCTYPE svg><svg/>").is_ok());
        assert!(matches!(
            check_svg("<html></html>"),
            Err(DiagramError::InvalidSvg)
        ));
        assert!(matches!(check_svg(""), Err(DiagramError::InvalidSvg)));
        assert!(matches!(
            check_svg("<?xml unterminated <svg>"),
            Err(DiagramError::InvalidSvg)
        ));
    }

    #[test]
    fn font_family_with_quote_entities_is_read_whole() {
        let ok = r#"<style>#m{font-family: &quot;Libertinus Serif&quot;, serif;}</style>"#;
        assert!(font_problems(ok).is_empty(), "{:?}", font_problems(ok));
        let bad = r#"<style>#m{font-family: &quot;Comic Sans&quot;, cursive;}</style>"#;
        assert_eq!(font_problems(bad).len(), 1);
    }

    #[test]
    fn generic_only_font_attributes_are_pointed_at_embedded_fonts() {
        let gantt =
            r#"<g class="grid" font-size="10" font-family="sans-serif" text-anchor="middle"/>"#;
        assert_eq!(font_problems(gantt).len(), 1);
        let fixed = normalize_fonts(gantt);
        assert!(font_problems(&fixed).is_empty(), "{fixed}");
        assert!(fixed.contains("font-size=\"10\""), "other attributes stay");
        // Declarations that already name a face are left alone.
        let named = r#"<text font-family="Arial, sans-serif">x</text>"#;
        assert_eq!(normalize_fonts(named), named);
    }

    #[test]
    fn font_family_must_be_embedded() {
        let ok = r#"<svg><style>text{font-family:"Libertinus Serif",serif;}</style></svg>"#;
        assert!(font_problems(ok).is_empty(), "{:?}", font_problems(ok));
        let attr = r#"<svg><text font-family="DejaVu Sans Mono">x</text></svg>"#;
        assert!(font_problems(attr).is_empty());
        let attr_with_more =
            r##"<text font-family="Libertinus Serif" font-size="22.4" fill="#2E1A5C">x</text>"##;
        assert!(
            font_problems(attr_with_more).is_empty(),
            "{:?}",
            font_problems(attr_with_more)
        );
        let bad_attr = r#"<text font-family="Arial" font-size="12">x</text>"#;
        assert_eq!(font_problems(bad_attr), vec!["Arial".to_string()]);
        let bad =
            r#"<svg><style>g{font-family:"trebuchet ms",verdana,arial,sans-serif;}</style></svg>"#;
        assert_eq!(font_problems(bad).len(), 1);
        assert!(
            check_svg(bad).is_ok(),
            "font problems are warnings, not errors"
        );
        assert_eq!(check_svg(bad).unwrap().len(), 1);
    }

    #[test]
    fn diagram_kind_skips_comments_init_and_frontmatter() {
        assert_eq!(diagram_kind("graph TD; A-->B"), "graph");
        assert_eq!(
            diagram_kind(
                "
  flowchart LR
 A-->B"
            ),
            "flowchart"
        );
        assert_eq!(
            diagram_kind(
                "%% a comment
%%{init: {}}%%
sequenceDiagram
 A->>B: x"
            ),
            "sequenceDiagram"
        );
        assert_eq!(
            diagram_kind(
                "---
title: T
---
classDiagram
 class A"
            ),
            "classDiagram"
        );
        assert_eq!(
            diagram_kind(
                "stateDiagram-v2
 [*] --> A"
            ),
            "stateDiagram-v2"
        );
        assert_eq!(diagram_kind("pie title Pets"), "pie");
        assert_eq!(diagram_kind(""), "");
    }

    #[test]
    fn routing_sends_types_to_the_right_backend() {
        struct Named(&'static str);
        impl DiagramRenderer for Named {
            fn render(&self, _s: &str, _t: &Theme) -> Result<String, DiagramError> {
                Ok(format!("<svg>{}</svg>", self.0))
            }
            fn name(&self) -> String {
                self.0.to_string()
            }
        }
        let r = RoutingRenderer {
            primary: Box::new(Named("native")),
            fallback: Box::new(Named("external")),
            kinds: NATIVE_KINDS.iter().map(|k| k.to_string()).collect(),
        };
        let t = Theme::embedded();
        assert_eq!(
            r.render("graph TD; A-->B", &t).unwrap(),
            "<svg>native</svg>"
        );
        assert_eq!(
            r.render(
                "stateDiagram-v2
 [*] --> A",
                &t
            )
            .unwrap(),
            "<svg>native</svg>"
        );
        assert_eq!(
            r.render(
                "sequenceDiagram
 A->>B: x",
                &t
            )
            .unwrap(),
            "<svg>external</svg>"
        );
        assert_eq!(
            r.render(
                "erDiagram
 A ||--o{ B : x",
                &t
            )
            .unwrap(),
            "<svg>external</svg>"
        );
        assert_eq!(
            r.render(
                "pie
 \"a\": 1",
                &t
            )
            .unwrap(),
            "<svg>external</svg>"
        );
        assert!(r.name().contains("native") && r.name().contains("external"));
    }

    #[test]
    fn collector_deduplicates_by_source() {
        let mut c = DiagramCollector::default();
        let a = c.request("p1.md", 3, "graph TD; A-->B");
        let b = c.request("p2.md", 9, "graph TD; A-->B");
        let d = c.request("p2.md", 20, "graph LR; X-->Y");
        assert_eq!(a, b);
        assert_ne!(a, d);
        assert_eq!(c.sources.len(), 2);
        assert_eq!(c.occurrences[&a].len(), 2);
    }

    struct Fake {
        calls: std::sync::atomic::AtomicUsize,
        result: fn(&str) -> Result<String, DiagramError>,
    }

    impl DiagramRenderer for Fake {
        fn render(&self, source: &str, _theme: &Theme) -> Result<String, DiagramError> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            (self.result)(source)
        }
        fn name(&self) -> String {
            "fake".into()
        }
    }

    fn fake(result: fn(&str) -> Result<String, DiagramError>) -> Fake {
        Fake {
            calls: Default::default(),
            result,
        }
    }

    #[test]
    fn test_diagram_cache_hit_skips_backend() {
        let dir = tempfile::tempdir().unwrap();
        let theme = Theme::embedded();
        let r = fake(|_| Ok("<svg>one</svg>".into()));
        let (svg, _) = render_cached(&r, &theme, Some(dir.path()), "graph TD; A-->B").unwrap();
        assert_eq!(svg, "<svg>one</svg>");
        let (again, _) = render_cached(&r, &theme, Some(dir.path()), "graph TD; A-->B").unwrap();
        assert_eq!(again, "<svg>one</svg>");
        assert_eq!(r.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Different source or theme: backend is called again.
        render_cached(&r, &theme, Some(dir.path()), "graph TD; X-->Y").unwrap();
        let other_theme = Theme { json: "{}".into() };
        render_cached(&r, &other_theme, Some(dir.path()), "graph TD; A-->B").unwrap();
        assert_eq!(r.calls.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn test_diagram_failure_names_page_and_line() {
        let mut c = DiagramCollector::default();
        let source = "graph TD; A-->B and a very long first line that goes past the sixty character limit for sure";
        c.request("required/01_intro.md", 47, source);
        let r = fake(|_| Err(DiagramError::RenderFailed("boom\nsecond line".into())));
        let out = render_all(&c, &r, &Theme::embedded(), None);
        assert_eq!(out.errors.len(), 1);
        let e = &out.errors[0];
        assert_eq!(e.code, ErrorCode::E004);
        assert_eq!(e.page.as_deref(), Some("required/01_intro.md"));
        assert_eq!(e.line, Some(47));
        assert!(
            e.message
                .starts_with("diagram failed to render\n  first line: graph TD; A-->B")
        );
        let first_line = e
            .message
            .lines()
            .nth(1)
            .unwrap()
            .trim_start_matches("  first line: ");
        assert_eq!(
            first_line.chars().count(),
            60,
            "first line is truncated to 60 characters"
        );
        assert!(e.message.contains("  backend: fake"));
        assert!(e.message.contains("  reason: boom"));
        assert!(out.svgs.is_empty());
    }

    #[test]
    fn empty_diagram_is_an_error() {
        let mut c = DiagramCollector::default();
        c.request("p.md", 1, "  \n");
        let r = fake(|_| Ok("<svg/>".into()));
        let out = render_all(&c, &r, &Theme::embedded(), None);
        assert_eq!(out.errors.len(), 1);
        assert_eq!(r.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn render_all_stops_after_first_error_in_source_order() {
        let mut c = DiagramCollector::default();
        c.request("a.md", 1, "graph A");
        c.request("b.md", 2, "graph A");
        c.request("a.md", 5, "graph B");
        let r = fake(|s| Ok(format!("<svg>{s}</svg>")));
        let out = render_all(&c, &r, &Theme::embedded(), None);
        assert!(out.errors.is_empty());
        assert_eq!(out.svgs.len(), 2);
        assert_eq!(r.calls.load(std::sync::atomic::Ordering::SeqCst), 2);

        let bad = fake(|_| Err(DiagramError::Timeout(30)));
        let out = render_all(&c, &bad, &Theme::embedded(), None);
        assert_eq!(out.errors.len(), 1);
        assert_eq!(out.errors[0].page.as_deref(), Some("a.md"));
        assert_eq!(out.errors[0].line, Some(1));
        assert_eq!(
            bad.calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "later diagrams must not be rendered"
        );
    }

    #[test]
    fn foreign_object_from_backend_fails_render() {
        let mut c = DiagramCollector::default();
        c.request("p.md", 1, "graph A");
        let r = fake(|_| Ok("<svg><foreignObject/></svg>".into()));
        let out = render_all(&c, &r, &Theme::embedded(), None);
        assert_eq!(out.errors.len(), 1);
        assert!(out.errors[0].message.contains("htmlLabels"));
    }

    #[test]
    fn embedded_theme_is_valid_json_with_font_and_no_html_labels() {
        let v: serde_json::Value = serde_json::from_str(&Theme::embedded().json).unwrap();
        assert_eq!(v["themeVariables"]["fontFamily"], "Readex Pro");
        // mermaid 11+ (mmdc 12) only honours the top-level key; older versions the nested one.
        assert_eq!(v["fontFamily"], "Readex Pro");
        // Without a seed, class and ER diagrams get random outline jitter on every run.
        assert_eq!(v["handDrawnSeed"], 1);
        assert_eq!(v["htmlLabels"], false);
        assert_eq!(v["flowchart"]["htmlLabels"], false);
    }

    #[test]
    fn test_theme_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let valid_path = dir.path().join("theme.json");
        std::fs::write(&valid_path, "{\"key\": \"val\"}").unwrap();
        assert!(Theme::from_file(&valid_path).is_ok());

        let invalid_path = dir.path().join("bad.json");
        std::fs::write(&invalid_path, "not json").unwrap();
        assert!(Theme::from_file(&invalid_path).is_err());

        let missing = dir.path().join("missing.json");
        assert!(Theme::from_file(&missing).is_err());
    }

    #[test]
    fn test_check_svg_unclosed_comment_and_doctype() {
        assert!(matches!(
            check_svg("<!-- unclosed"),
            Err(DiagramError::InvalidSvg)
        ));
        assert!(matches!(
            check_svg("<!DOCTYPE unclosed"),
            Err(DiagramError::InvalidSvg)
        ));
    }

    #[test]
    fn test_font_problems_syntax_variants() {
        assert!(font_problems("<text font-family x>").is_empty());
        assert!(font_problems("<text font-family=Arial>").is_empty());
    }

    #[test]
    fn test_routing_renderer_cache_id() {
        struct MockBackend;
        impl DiagramRenderer for MockBackend {
            fn render(&self, _s: &str, _t: &Theme) -> Result<String, DiagramError> {
                Ok("<svg></svg>".into())
            }
            fn name(&self) -> String {
                "mock".into()
            }
            fn cache_id(&self) -> String {
                "id1".into()
            }
        }
        let r = RoutingRenderer {
            primary: Box::new(MockBackend),
            fallback: Box::new(MockBackend),
            kinds: vec!["k1".into()],
        };
        assert!(r.cache_id().contains("routing[k1]"));
    }

    #[test]
    fn test_render_all_font_warning() {
        let mut c = DiagramCollector::default();
        c.request("p.md", 1, "graph A");
        let r = fake(|_| Ok("<svg><text font-family=\"Arial\">x</text></svg>".into()));
        let out = render_all(&c, &r, &Theme::embedded(), None);
        assert_eq!(out.warnings.len(), 1);
        assert_eq!(out.warnings[0].code, WarningCode::W010);
    }
}
