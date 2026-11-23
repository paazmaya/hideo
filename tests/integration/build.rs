//! End-to-end builds: page structure, links, page numbers, reproducibility, edge cases.

use std::collections::BTreeSet;

use lopdf::Object;

use crate::common::*;

#[test]
fn test_minimal_build_has_cover_contents_and_pages() {
    let (out, r) = build_fixture("minimal", &["-q"]);
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    assert_eq!(page_count(&doc), 4, "cover + contents + 2 pages");
    let cover = page_text(&doc, 1);
    assert!(
        cover.contains("Minimal Book")
            && cover.contains("Required course material")
            && cover.contains("Test Author"),
        "{cover}"
    );
    assert!(page_text(&doc, 3).contains("Welcome"));
    assert!(page_text(&doc, 4).contains("Another Section"));
}

#[test]
fn test_empty_tier_subtitle_is_omitted_from_cover() {
    let input = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), input.path());
    let config = std::fs::read_to_string(input.path().join("hideo.toml")).unwrap();
    std::fs::write(
        input.path().join("hideo.toml"),
        format!("{config}\n[tier.required]\nsubtitle = \"\"\n"),
    )
    .unwrap();

    let output = tempfile::tempdir().unwrap();
    let result = hideo()
        .args(["build", "--input"])
        .arg(input.path())
        .arg("--output")
        .arg(output.path())
        .args(["-q"])
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", stderr(&result));
    let doc = load(&output.path().join("required.pdf"));
    let cover = page_text(&doc, 1);
    assert!(!cover.contains("Required course material"), "{cover}");
}

#[test]
fn test_page_number_on_all_but_cover() {
    let (out, r) = build_fixture("minimal", &["-q"]);
    assert!(r.status.success());
    let doc = load(&out.path().join("required.pdf"));
    let cover = page_text(&doc, 1);
    assert!(
        !cover.chars().any(|c| c.is_ascii_digit()),
        "the cover has no page number: {cover:?}"
    );
    for n in 2..=4u32 {
        let text = page_text(&doc, n);
        assert!(
            text.trim_end().ends_with(&n.to_string()),
            "page {n} footer: {:?}",
            text.trim_end()
        );
    }
}

#[test]
fn test_contents_page_numbers_match_actual_pages() {
    let (out, r) = build_fixture("full", &["--diagrams", "off", "-q"]);
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    let contents = page_text(&doc, 2);
    let folder = contents.find("Sub").expect("folder group in contents");
    let nested = contents
        .find("Nested Page")
        .expect("nested page in contents");
    assert!(
        folder < nested,
        "folder group precedes its child page: {contents}"
    );
    // (contents title, text that only appears on that page's body)
    let entries = [
        ("Introduction", "This is the introduction page"),
        ("Headings and Text", "A second-level heading"),
        ("Code Examples", "Rust Code"),
        ("Tables", "Simple Table"),
        ("Images", "A body image scaled"),
        ("Nested Page", "Unicode headings"),
        ("Deep Page", "deeper directory levels"),
        ("Another Deep Page", "second page must remain"),
        ("Third Deep Page", "third page must also remain"),
        ("Final Page", "same directory as a deeper subfolder"),
    ];
    let pages = page_count(&doc) as u32;
    for (title, marker) in entries {
        let actual = (3..=pages)
            .find(|&n| page_text(&doc, n).contains(marker))
            .unwrap_or_else(|| panic!("no page shows {title}"));
        let idx = contents
            .find(title)
            .unwrap_or_else(|| panic!("{title} not in contents: {contents}"));
        let after = &contents[idx + title.len()..];
        // Skip the dotted leader (its dots are text in the content stream) and whitespace.
        let shown: u32 = after
            .trim_start_matches(|c: char| c == '.' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or_else(|_| panic!("no number after {title}: {after:?}"));
        assert_eq!(
            shown, actual,
            "contents says {title} is on page {shown}, it is on {actual}"
        );
    }
}

fn annotations(doc: &lopdf::Document) -> Vec<lopdf::Dictionary> {
    let mut all = Vec::new();
    for (_, page_id) in doc.get_pages() {
        for annot in doc.get_page_annotations(page_id).unwrap_or_default() {
            if annot.get(b"Subtype").ok().and_then(name_of).as_deref() == Some("Link") {
                all.push(annot.clone());
            }
        }
    }
    all
}

fn action(doc: &lopdf::Document, annot: &lopdf::Dictionary) -> Option<lopdf::Dictionary> {
    match annot.get(b"A").ok()? {
        Object::Reference(id) => doc.get_dictionary(*id).ok().cloned(),
        Object::Dictionary(d) => Some(d.clone()),
        _ => None,
    }
}

#[test]
fn test_internal_links_become_goto_annotations() {
    let (out, r) = build_fixture("minimal", &["-q"]);
    assert!(r.status.success());
    let doc = load(&out.path().join("required.pdf"));
    let links = annotations(&doc);
    let internal = links
        .iter()
        .filter(|a| {
            a.has(b"Dest")
                || action(&doc, a).is_some_and(|act| {
                    act.get(b"S").ok().and_then(name_of).as_deref() == Some("GoTo")
                })
        })
        .count();
    // Contents: 2 entries; page 1 has two links; page 2 has one.
    assert!(
        internal >= 5,
        "internal links: {internal} of {}",
        links.len()
    );
    assert_eq!(internal, links.len(), "no external links in this fixture");
}

#[test]
fn test_external_links_become_uri_annotations() {
    let (out, r) = build_fixture("full", &["--diagrams", "off", "-q"]);
    assert!(r.status.success());
    let doc = load(&out.path().join("required.pdf"));
    let mut uris = BTreeSet::new();
    for a in annotations(&doc) {
        if let Some(act) = action(&doc, &a)
            && let Ok(uri) = act.get(b"URI").and_then(|u| u.as_str())
        {
            uris.insert(String::from_utf8_lossy(uri).into_owned());
        }
    }
    assert!(uris.contains("https://example.com/page"), "{uris:?}");
    assert!(uris.contains("mailto:someone@example.com"), "{uris:?}");
}

#[test]
fn test_reproducible_two_builds_identical() {
    // Separate processes with different worker counts (plan F19).
    for name in ["repro", "full"] {
        let (a, ra) = build_fixture(name, &["--jobs", "1", "--diagrams", "off", "-q"]);
        let (b, rb) = build_fixture(name, &["--jobs", "8", "--diagrams", "off", "-q"]);
        assert!(
            ra.status.success() && rb.status.success(),
            "{}{}",
            stderr(&ra),
            stderr(&rb)
        );
        let mut compared = 0;
        for entry in std::fs::read_dir(a.path()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "pdf") {
                let other = b.path().join(path.file_name().unwrap());
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    std::fs::read(&other).unwrap(),
                    "{} differs",
                    path.display()
                );
                compared += 1;
            }
        }
        assert!(compared >= 1);
    }
}

#[test]
fn test_timestamp_from_source_date_epoch() {
    let build = |epoch: Option<&str>| {
        let out = tempfile::tempdir().unwrap();
        let mut cmd = hideo();
        if let Some(e) = epoch {
            cmd.env("SOURCE_DATE_EPOCH", e);
        }
        let r = cmd
            .args(["build", "-q", "--input"])
            .arg(fixture("repro"))
            .arg("--output")
            .arg(out.path())
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", stderr(&r));
        std::fs::read(out.path().join("required.pdf")).unwrap()
    };
    assert_eq!(build(Some("1700000000")), build(Some("1700000000")));
    assert_ne!(
        build(Some("1700000000")),
        build(Some("1700000001")),
        "the timestamp is part of the PDF"
    );
    assert_eq!(
        build(None),
        build(Some("946684800")),
        "default is 2000-01-01T00:00:00Z"
    );
}

#[test]
fn test_windows_paths_fixture_resolves_links_identically_on_every_os() {
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(fixture("windows-paths"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let warnings = v["warnings"].as_array().unwrap();
    let find = |code: &str| -> Vec<&serde_json::Value> {
        warnings.iter().filter(|w| w["code"] == code).collect()
    };
    // `MIXED%20case.md` resolves with a case warning on every platform (F7).
    let w005 = find("W005");
    assert_eq!(w005.len(), 1, "{warnings:?}");
    assert!(
        w005[0]["message"]
            .as_str()
            .unwrap()
            .contains("MIXED%20case.md")
    );
    // `C:\x\y.md` is not a scheme: it is an unresolved page, not an external link.
    let w001 = find("W001");
    assert_eq!(w001.len(), 1, "{warnings:?}");
    assert!(
        w001[0]["message"].as_str().unwrap().contains("C:"),
        "{w001:?}"
    );
    // Everything else resolved: %20, mixed case, nested, back slashes, up-and-down with anchor.
}

#[test]
fn test_windows_paths_fixture_builds() {
    let (out, r) = build_fixture("windows-paths", &["-q"]);
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    assert_eq!(page_count(&doc), 2 + 4);
}

#[test]
fn test_edge_fixture_unresolved_and_placeholder_warnings() {
    let out = hideo()
        .args(["check", "--input"])
        .arg(fixture("edge"))
        .output()
        .unwrap();
    let text = stdout(&out);
    assert!(
        text.contains("unresolved link `nope.md` (page not found)"),
        "{text}"
    );
    assert!(
        text.contains("unresolved link `#nope` (heading not found)"),
        "{text}"
    );
    assert!(
        text.contains("unresolved link `nowhere/` (file not found)"),
        "{text}"
    );
    assert!(
        text.contains("hero image `images/not-there.png` not found"),
        "{text}"
    );
    assert!(text.contains("Unresolved links in required.pdf"), "{text}");
}

#[test]
fn test_no_partial_pdf_on_error() {
    let (out, r) = build_fixture("edge", &["-q"]);
    assert_eq!(r.status.code(), Some(1));
    let leftovers: Vec<String> = std::fs::read_dir(out.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !leftovers
            .iter()
            .any(|n| n.ends_with(".pdf") || n.starts_with(".tmp")),
        "{leftovers:?}"
    );
}

#[test]
fn test_cache_is_written_and_reused() {
    let out = tempfile::tempdir().unwrap();
    let run = || {
        let r = hideo()
            .args(["build", "-q", "--input"])
            .arg(fixture("minimal"))
            .arg("--output")
            .arg(out.path())
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", stderr(&r));
        std::fs::read(out.path().join("required.pdf")).unwrap()
    };
    let first = run();
    let images = out.path().join(".hideo-cache/images");
    assert!(
        std::fs::read_dir(&images).unwrap().count() > 0,
        "scaled images are cached"
    );
    assert_eq!(first, run(), "warm build gives identical bytes");
}

#[test]
fn test_check_warns_about_uncovered_glyphs_before_the_build_fails() {
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(fixture("edge"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let w011: Vec<_> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["code"] == "W011")
        .collect();
    assert_eq!(w011.len(), 1, "only the Japanese heading is flagged: {v}");
    assert_eq!(w011[0]["page"], "required/01_edge.md");
    assert_eq!(w011[0]["line"], 14);
    assert!(w011[0]["message"].as_str().unwrap().contains('日'));
    // The Finnish and Swedish letters of the full fixture are covered.
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(fixture("full"))
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["code"] != "W011"),
        "{v}"
    );
}

/// x positions (in points, from the page's left edge) of every text item whose text starts with
/// `prefix`, walking nested groups.
fn text_x_positions(frame: &typst::layout::Frame, origin: f64, prefix: &str, out: &mut Vec<f64>) {
    use typst::layout::FrameItem;
    for (pos, item) in frame.items() {
        let x = origin + pos.x.to_pt();
        match item {
            FrameItem::Text(t) if t.text.starts_with(prefix) => out.push(x),
            FrameItem::Group(g) => {
                text_x_positions(&g.frame, x + g.transform.tx.to_pt(), prefix, out)
            }
            _ => {}
        }
    }
}

#[test]
fn test_contents_two_columns() {
    use camino::Utf8PathBuf;
    use hideo::build::{BuildOptions, Mode, Options, generate};
    use hideo::compile::{compile_document, source_date};
    use hideo::config::{Config, DiagramBackendConfig};
    use hideo::world::HideoWorld;

    // 80 tiny pages: more entries than fit in one column.
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    for n in 3..=80 {
        std::fs::write(
            dir.path().join(format!("required/{n:03}_page.md")),
            format!(
                "---
title: Entry {n}
hero: images/hero.png
---
# Entry {n}

text
"
            ),
        )
        .unwrap();
    }
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let config = Config::load(&root.join("hideo.toml")).unwrap();
    let today = source_date(Some("1700000000")).unwrap();
    let options = Options {
        source_dir: root.clone(),
        tiers: vec!["required".into()],
        verbose: false,
        mode: Mode::Build(BuildOptions {
            output: root.join("out"),
            keep_typst: None,
            diagrams: DiagramBackendConfig::Off,
            today,
        }),
    };
    let generated = generate(&config, &options, "required").unwrap();
    let world = HideoWorld::new(&generated.files, today).unwrap();
    let document = compile_document(&world).output.unwrap();
    let mut xs = Vec::new();
    text_x_positions(&document.pages()[1].frame, 0.0, "Entry ", &mut xs);
    assert!(
        xs.len() >= 70,
        "the contents page lists the entries: {}",
        xs.len()
    );
    let (min, max) = xs
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
    let text_width = (config.page.width_mm - 2.0 * config.page.margin_x_mm) * 2.834_645_669;
    assert!(
        max - min > text_width / 3.0,
        "entries start in two columns: x ranges from {min:.0} to {max:.0} pt (text width {text_width:.0} pt)"
    );
}
