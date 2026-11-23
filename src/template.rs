//! Generation of the virtual Typst files: `template.typ`, `main.typ` and one file per page
//! (plan 7.2 step 9, 7.9).

use crate::compile::SourceMap;
use crate::config::{Config, capitalize};
use crate::markdown::emit::PageMarkup;
use crate::markdown::escape::{Position, escape_text, typst_string};
use crate::page::Page;

pub const TEMPLATE_SOURCE: &str = include_str!("../assets/template.typ");
pub const ARROW_SVG: &[u8] = include_bytes!("../assets/arrow.svg");

#[derive(Debug, Clone)]
pub struct FolderHeading {
    pub level: usize,
    pub title: String,
    pub key: String,
}

/// Number as Typst source: `210` and `118.125`, never exponent notation.
fn num(n: f64) -> String {
    format!("{n}")
}

/// `template.typ` with the configuration tokens filled in.
pub fn template_typ(config: &Config) -> String {
    TEMPLATE_SOURCE
        .replace("{{MARGIN_X_MM}}", &num(config.page.margin_x_mm))
        .replace("{{MARGIN_Y_MM}}", &num(config.page.margin_y_mm))
        .replace("{{PAGE_WIDTH_MM}}", &num(config.page.width_mm))
}

/// PDF title: the book title plus the tier (plan 7.10 item 2).
pub fn document_title(config: &Config, tier: &str) -> String {
    format!("{} \u{2013} {}", config.cover.title, capitalize(tier))
}

/// A page with its resolved hero image, in PDF order.
pub struct PageEntry<'a> {
    pub page: &'a Page,
    /// Folder headings that first become active immediately before this page.
    pub folders: Vec<FolderHeading>,
    /// Virtual path of the hero (real image or placeholder).
    pub hero: Option<String>,
    /// Index into the page list, 1-based, used for the file name `pages/001.typ`.
    pub number: usize,
}

pub fn page_vpath(number: usize) -> String {
    format!("pages/{number:03}.typ")
}

fn folder_depth(rel_path: &str) -> usize {
    rel_path.split('/').count().saturating_sub(2)
}

fn navigation_title(page: &Page) -> &str {
    page.headings
        .iter()
        .find(|heading| heading.level == 1)
        .map_or_else(|| page.title(), |heading| heading.text.as_str())
}

/// `main.typ`: document setup, cover, contents, includes.
pub fn main_typ(
    config: &Config,
    tier: &str,
    entries: &[PageEntry],
    cover: &str,
    contents_hero: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str("#import \"/template.typ\": *\n");
    out.push_str(&format!(
        "#show: book.with(lang: {}, title: {}, author: {})\n",
        typst_string(&config.book.lang),
        typst_string(&document_title(config, tier)),
        typst_string(&config.cover.author),
    ));
    out.push_str(&format!(
        "#cover({}, {}, {}, {}, {}mm, {}mm)\n",
        typst_string(cover),
        typst_string(&config.cover.title),
        typst_string(&config.tier_subtitle(tier)),
        typst_string(&config.cover.author),
        num(config.cover.panel_width_mm),
        num(config.cover.panel_margin_mm),
    ));

    let hero = contents_hero
        .map(typst_string)
        .unwrap_or_else(|| "none".to_string());
    out.push_str(&format!(
        "#contents({hero}, {}, {})\n",
        typst_string(&config.contents.title),
        config.contents.columns
    ));

    for e in entries {
        out.push_str(&format!(
            "#include {}\n",
            typst_string(&page_vpath(e.number))
        ));
    }
    out
}

/// One page file and its source map. Preamble lines map to Markdown line 1.
pub fn page_typ(config: &Config, entry: &PageEntry, markup: &PageMarkup) -> (String, SourceMap) {
    let page = entry.page;
    let fm = &page.frontmatter;
    let mut lines: Vec<(String, u32)> = Vec::new();
    let mut push = |text: String, md: u32| lines.push((text, md));

    push("#import \"/template.typ\": *".to_string(), 1);
    push("#pagebreak(weak: true)".to_string(), 1);
    push(format!("#metadata(none) <{}>", page.key), 1);

    for folder in &entry.folders {
        push(
            format!(
                "#place(top + left, hide[#heading(level: {}, supplement: [hideo-folder])[{}] <folder--{}>])",
                folder.level,
                escape_text(&folder.title, Position::LineStart),
                folder.key,
            ),
            1,
        );
    }
    push(
        format!(
            "#place(top + left, hide[#heading(level: {}, supplement: [hideo-page])[{}] <page--{}>])",
            folder_depth(&page.rel_path) + 1,
            escape_text(navigation_title(page), Position::LineStart),
            page.key,
        ),
        1,
    );

    let mut args = Vec::new();
    if let Some(hero) = &entry.hero {
        args.push(format!("hero: {}", typst_string(hero)));
        if let Some(alt) = fm.hero_alt.as_deref().filter(|a| !a.trim().is_empty()) {
            args.push(format!("hero-alt: {}", typst_string(alt)));
        }
    }
    if let Some(s) = fm.subtitle.as_deref().filter(|s| !s.trim().is_empty()) {
        args.push(format!("subtitle: {}", typst_string(s)));
    }
    if let Some(a) = fm.author.as_deref().filter(|a| !a.trim().is_empty()) {
        args.push(format!("author: {}", typst_string(a)));
    }
    if !args.is_empty() {
        push(format!("#page-head({})", args.join(", ")), 1);
    }

    // Every page has a heading (outline entry, navigation): use the title if the body has none.
    if page.headings.is_empty() {
        push(
            format!(
                "#heading(outlined: false, bookmarked: false, level: 1)[{}] <{}--page-title>",
                escape_text(page.title(), Position::LineStart),
                page.key
            ),
            1,
        );
    }

    let wrap_lang = fm.lang.as_deref().filter(|l| *l != config.book.lang);
    if let Some(lang) = wrap_lang {
        push("#[".to_string(), 1);
        push(format!("#set text(lang: {})", typst_string(lang)), 1);
    }
    if !markup.text.is_empty() {
        for (text, md) in markup.text.split('\n').zip(markup.line_map.iter()) {
            push(text.to_string(), *md);
        }
    }
    if wrap_lang.is_some() {
        let last = markup.line_map.last().copied().unwrap_or(1);
        push("]".to_string(), last);
    }

    let text = lines
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let map = SourceMap {
        page: page.rel_path.clone(),
        lines: lines.iter().map(|(_, l)| *l).collect(),
    };
    (text, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::SourcePage;
    use crate::frontmatter::Frontmatter;
    use camino::{Utf8Path, Utf8PathBuf};

    fn config() -> Config {
        Config::from_toml(
            "tiers=[\"req\"]\n[book]\nlang=\"fi\"\n[cover]\ntitle=\"My \\\"Book\\\"\"\nauthor=\"A. Author\"\n",
            Utf8Path::new("."),
        )
        .unwrap()
    }

    fn page(rel: &str, key: &str, fm: Frontmatter, body: &str) -> Page {
        Page::build(
            SourcePage {
                tier: "req".into(),
                rel_path: rel.into(),
                abs_path: Utf8PathBuf::from(rel),
                frontmatter: fm,
                body: body.into(),
            },
            key.into(),
        )
        .unwrap()
    }

    fn fm(title: &str) -> Frontmatter {
        Frontmatter {
            title: Some(title.into()),
            hero: Some("h.png".into()),
            ..Default::default()
        }
    }

    #[test]
    fn tokens_are_filled() {
        let t = template_typ(&config());
        assert!(!t.contains("{{"), "unfilled token");
        assert!(t.contains("#let margin-x = 20mm"));
        assert!(t.contains("#let page-width = 210mm"));
    }

    #[test]
    fn main_typ_has_cover_contents_and_includes() {
        let cfg = config();
        let a = page("req/01.md", "req-01", fm("First: \"A\""), "text");
        let b = page("req/sub/02.md", "req-sub-02", fm("Second"), "text");
        let entries = vec![
            PageEntry {
                page: &a,
                folders: vec![],
                hero: None,
                number: 1,
            },
            PageEntry {
                page: &b,
                folders: vec![],
                hero: None,
                number: 2,
            },
        ];
        let main = main_typ(&cfg, "req", &entries, "images/c.jpg", Some("images/ch.jpg"));
        assert!(main.contains(r#"title: "My \"Book\" – Req""#), "{main}");
        assert!(main.contains(r#"lang: "fi""#));
        assert!(main.contains(r#"#cover("images/c.jpg", "My \"Book\"", "Req course material", "A. Author", 170mm, 20mm)"#), "{main}");
        assert!(main.contains(r#"#contents("images/ch.jpg", "Contents", 2)"#));
        assert!(main.contains("#include \"pages/001.typ\"\n#include \"pages/002.typ\""));
    }

    #[test]
    fn main_typ_uses_tier_specific_subtitle() {
        let mut cfg = config();
        cfg.tier.insert(
            "req".into(),
            crate::config::TierConfig {
                subtitle: Some("Custom cover subtitle".into()),
            },
        );
        let main = main_typ(&cfg, "req", &[], "images/c.jpg", None);
        assert!(
            main.contains(r#"#cover("images/c.jpg", "My \"Book\"", "Custom cover subtitle""#),
            "{main}"
        );
    }

    #[test]
    fn page_typ_head_title_and_line_map() {
        let cfg = config();
        let mut f = fm("Fallback title");
        f.subtitle = Some("Sub".into());
        f.author = Some("Me".into());
        f.hero_alt = Some("A view".into());
        let p = page("req/01.md", "req-01", f, "body line");
        let entry = PageEntry {
            page: &p,
            folders: vec![],
            hero: Some("images/h.jpg".into()),
            number: 1,
        };
        let markup = PageMarkup {
            text: "one\n\ntwo".into(),
            line_map: vec![8, 9, 10],
        };
        let (text, map) = page_typ(&cfg, &entry, &markup);
        assert!(text.contains("#metadata(none) <req-01>"));
        assert!(text.contains(
            "#place(top + left, hide[#heading(level: 1, supplement: [hideo-page])[Fallback title] <page--req-01>])"
        ));
        assert!(text.contains(r#"#page-head(hero: "images/h.jpg", hero-alt: "A view", subtitle: "Sub", author: "Me")"#), "{text}");
        assert!(
            text.contains("#heading(outlined: false, bookmarked: false, level: 1)[Fallback title] <req-01--page-title>"),
            "page without headings gets the title"
        );
        assert_eq!(text.split('\n').count(), map.lines.len());
        assert_eq!(map.page, "req/01.md");
        let idx = text.split('\n').position(|l| l == "two").unwrap();
        assert_eq!(map.lines[idx], 10);
    }

    #[test]
    fn page_with_headings_does_not_get_title_heading_and_lang_wraps_body() {
        let cfg = config();
        let mut f = fm("T");
        f.lang = Some("en".into());
        let p = page("req/01.md", "req-01", f, "# Own heading\n");
        let entry = PageEntry {
            page: &p,
            folders: vec![],
            hero: None,
            number: 1,
        };
        let markup = PageMarkup {
            text: "#heading(level: 1)[Own heading] <x>".into(),
            line_map: vec![5],
        };
        let (text, _) = page_typ(&cfg, &entry, &markup);
        assert!(!text.contains("page-title"));
        assert!(
            text.contains("#[\n#set text(lang: \"en\")\n#heading"),
            "{text}"
        );
        assert!(text.ends_with("\n]"));
        // same language as the book: no wrapper
        let mut f = fm("T");
        f.lang = Some("fi".into());
        let p = page("req/01.md", "req-01", f, "# H\n");
        let entry = PageEntry {
            page: &p,
            folders: vec![],
            hero: None,
            number: 1,
        };
        let (text, _) = page_typ(&cfg, &entry, &markup);
        assert!(!text.contains("#set text(lang"));
    }

    #[test]
    fn page_numbers_are_zero_padded() {
        assert_eq!(page_vpath(7), "pages/007.typ");
        assert_eq!(page_vpath(1234), "pages/1234.typ");
    }

    #[test]
    fn main_typ_with_none_contents_hero() {
        let cfg = config();
        let main = main_typ(&cfg, "req", &[], "images/c.jpg", None);
        assert!(main.contains("#contents(none, \"Contents\", 2)"), "{main}");
    }

    #[test]
    fn page_typ_adds_folder_groups() {
        let cfg = config();
        let p = page(
            "req/01 Chapter/Child/page.md",
            "req-01-child-page",
            fm("Nested"),
            "# Title\n",
        );
        let entry = PageEntry {
            page: &p,
            folders: vec![
                FolderHeading {
                    level: 1,
                    title: "01 Chapter".into(),
                    key: "req-01-chapter".into(),
                },
                FolderHeading {
                    level: 2,
                    title: "Child".into(),
                    key: "req-01-chapter-child".into(),
                },
            ],
            hero: None,
            number: 1,
        };
        let markup = PageMarkup {
            text: "#heading(level: 1)[Title] <req-01-child-page--title>".into(),
            line_map: vec![5],
        };
        let (text, _) = page_typ(&cfg, &entry, &markup);
        assert!(text.contains(
            "#place(top + left, hide[#heading(level: 1, supplement: [hideo-folder])[01 Chapter] <folder--req-01-chapter>])"
        ));
        assert!(text.contains(
            "#place(top + left, hide[#heading(level: 3, supplement: [hideo-page])[Title] <page--req-01-child-page>])"
        ));
        assert!(text.contains("#heading(level: 1)[Title]"), "{text}");
    }

    // ---- palette contrast (plan 7.10 item 8, Appendix B) ----------------------------------

    /// `#RRGGBB` as linear-light relative luminance (WCAG 2.x).
    fn luminance(hex: &str) -> f64 {
        let channel = |i: usize| {
            let v = u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap() as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2)
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// The colour a `#let name = rgb("#...")` constant in the template has.
    fn template_color(name: &str) -> String {
        let marker = format!("#let {name} = rgb(\"");
        let start = TEMPLATE_SOURCE
            .find(&marker)
            .unwrap_or_else(|| panic!("no #let {name} in template"))
            + marker.len();
        TEMPLATE_SOURCE[start..start + 7].to_string()
    }

    /// A fill used as a literal in the template; fails if the template no longer contains it.
    fn literal(hex: &str) -> String {
        assert!(
            TEMPLATE_SOURCE
                .to_lowercase()
                .contains(&format!("\"{}\"", hex.to_lowercase())),
            "{hex} not in the template"
        );
        hex.to_string()
    }

    #[test]
    fn test_palette_contrast_at_least_4_5() {
        const WHITE: &str = "#FFFFFF";
        let ink = template_color("ink");
        let quote = literal("#F0EBF8");
        let code = literal("#F3F0F8");
        let header = literal("#E9E3F6");
        let zebra = literal("#F5F3F9");
        // (foreground, background, ratio stated in Appendix B)
        let pairs = [
            ("body on white", ink.clone(), WHITE.to_string(), 14.92),
            (
                "h1 on white",
                template_color("h1-color"),
                WHITE.to_string(),
                16.47,
            ),
            (
                "h2 on white",
                template_color("h2-color"),
                WHITE.to_string(),
                11.60,
            ),
            (
                "h3 on white",
                template_color("h3-color"),
                WHITE.to_string(),
                8.78,
            ),
            (
                "footer/subtitle on white",
                template_color("muted"),
                WHITE.to_string(),
                5.40,
            ),
            (
                "link on white",
                template_color("link-color"),
                WHITE.to_string(),
                10.03,
            ),
            (
                "link on quote fill",
                template_color("link-color"),
                quote.clone(),
                8.57,
            ),
            (
                "lead-in on white",
                template_color("accent"),
                WHITE.to_string(),
                7.88,
            ),
            (
                "lead-in on quote fill",
                template_color("accent"),
                quote.clone(),
                6.74,
            ),
            ("quote text", ink.clone(), quote, 12.75),
            ("code text", ink.clone(), code, 13.23),
            ("table text on header fill", ink.clone(), header, 11.92),
            ("table text on zebra fill", ink, zebra, 13.55),
            (
                "cover white on panel",
                WHITE.to_string(),
                template_color("panel-color"),
                14.80,
            ),
        ];
        for (what, fg, bg, stated) in pairs {
            let ratio = contrast(&fg, &bg);
            assert!(
                ratio >= 4.5,
                "{what}: {fg} on {bg} is {ratio:.2}:1, AA needs 4.5:1"
            );
            assert!(
                (ratio - stated).abs() < 0.05,
                "{what}: computed {ratio:.2}, Appendix B says {stated}"
            );
        }
    }

    #[test]
    fn contrast_function_matches_known_values() {
        assert!((contrast("#000000", "#FFFFFF") - 21.0).abs() < 1e-9);
        assert!((contrast("#777777", "#FFFFFF") - 4.48).abs() < 0.01);
        assert!((contrast("#FFFFFF", "#FFFFFF") - 1.0).abs() < 1e-9);
    }
}
