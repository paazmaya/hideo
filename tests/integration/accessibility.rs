//! Accessibility (plan 7.10): inspect the structure tree of the generated PDFs.

use std::collections::BTreeSet;

use lopdf::Object;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::common::*;

/// Tables, header cells and body cells in the Markdown sources of a tier, counted by the
/// parser (never hard-coded).
fn markdown_tables(tier_dir: &std::path::Path) -> (usize, usize, usize) {
    let mut tables = 0;
    let mut header_cells = 0;
    let mut body_cells = 0;
    let mut stack = vec![tier_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                let text = std::fs::read_to_string(&path).unwrap();
                let body = text.splitn(3, "---").nth(2).unwrap_or("");
                let (mut in_head, mut cols) = (false, 0);
                for ev in Parser::new_ext(body, Options::ENABLE_TABLES) {
                    match ev {
                        Event::Start(Tag::Table(a)) => {
                            tables += 1;
                            cols = a.len();
                        }
                        Event::Start(Tag::TableHead) => in_head = true,
                        Event::End(TagEnd::TableHead) => {
                            in_head = false;
                            header_cells += cols;
                        }
                        Event::Start(Tag::TableRow) if !in_head => body_cells += cols,
                        _ => {}
                    }
                }
            }
        }
    }
    (tables, header_cells, body_cells)
}

fn full_required() -> (tempfile::TempDir, lopdf::Document) {
    let (out, r) = build_fixture("full", &["--diagrams", "off", "-q"]);
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    (out, doc)
}

/// The `/A` entry of a structure element is an array of attribute dictionaries.
fn table_scope(doc: &lopdf::Document, e: &StructElem) -> Option<String> {
    let attrs = e.dict.get(b"A").ok()?;
    let attrs = match attrs {
        Object::Reference(id) => doc.get_object(*id).ok()?,
        o => o,
    };
    let list: Vec<&Object> = match attrs {
        Object::Array(items) => items.iter().collect(),
        o => vec![o],
    };
    list.into_iter().find_map(|a| {
        let d = match a {
            Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok()?,
            o => o.as_dict().ok()?,
        };
        if name_of(d.get(b"O").ok()?)? == "Table" {
            name_of(d.get(b"Scope").ok()?)
        } else {
            None
        }
    })
}

fn cell_roles_below(elems: &[StructElem], index: usize) -> Vec<&str> {
    let depth = elems[index].depth;
    elems[index + 1..]
        .iter()
        .take_while(|c| c.depth > depth)
        .filter(|c| c.role == "TH" || c.role == "TD")
        .map(|c| c.role.as_str())
        .collect()
}

#[test]
fn test_table_header_cells_tagged_th_column_scope() {
    let (_out, doc) = full_required();
    let elems = struct_elems(&doc);
    let roles = count_roles(&elems);
    let (tables, header_cells, body_cells) = markdown_tables(&fixture("full").join("required"));
    assert!(tables >= 4, "fixture has tables: {tables}");

    assert_eq!(roles["Table"], tables, "one Table per Markdown table");
    assert_eq!(roles["THead"], tables, "one THead per table");
    assert_eq!(
        roles["TH"], header_cells,
        "every header cell is a TH, and nothing else is"
    );
    assert_eq!(roles["TD"], body_cells);

    for th in elems.iter().filter(|e| e.role == "TH") {
        assert_eq!(
            table_scope(&doc, th).as_deref(),
            Some("Column"),
            "TH without column scope: {:?}",
            th.dict
        );
    }
}

#[test]
fn test_table_body_cells_tagged_td_not_th() {
    let (_out, doc) = full_required();
    let elems = struct_elems(&doc);
    let mut heads = 0;
    for (i, e) in elems.iter().enumerate() {
        if e.role == "THead" {
            let cells = cell_roles_below(&elems, i);
            assert!(
                !cells.is_empty() && cells.iter().all(|r| *r == "TH"),
                "THead holds only TH: {cells:?}"
            );
            heads += 1;
        }
        if e.role == "TBody" {
            let cells = cell_roles_below(&elems, i);
            assert!(
                !cells.is_empty() && cells.iter().all(|r| *r == "TD"),
                "TBody holds only TD: {cells:?}"
            );
        }
    }
    assert!(heads >= 4);
}

#[test]
fn test_table_header_not_duplicated_in_struct_tree_across_pages() {
    let (_out, doc) = full_required();
    // The long table in 04_tables.md spans pages; its header is repeated visually...
    let pages_with_header: Vec<u32> = (1..=page_count(&doc) as u32)
        .filter(|&n| {
            let t = page_text(&doc, n);
            t.contains("Description") && t.contains("Status")
        })
        .collect();
    assert!(
        pages_with_header.len() >= 2,
        "header repeats on every page of the long table: {pages_with_header:?}"
    );
    // ...but the structure tree holds exactly one set of TH for it: the total equals the
    // number of header cells in the Markdown, however many pages the tables span.
    let th = struct_elems(&doc).iter().filter(|e| e.role == "TH").count();
    let (_, header_cells, _) = markdown_tables(&fixture("full").join("required"));
    assert_eq!(th, header_cells);
}

#[test]
fn test_table_negative_control_plain_row_is_td() {
    // A table built without `table.header` is all TD, so these tests can fail.
    use hideo::compile::{PdfSettings, SourceMaps, compile_pdf};
    use hideo::world::HideoWorld;
    use typst::foundations::Datetime;
    let src = "#set document(title: \"T\")\n#set text(lang: \"en\")\n#table(columns: 2, [A], [B], [1], [2])";
    let world = HideoWorld::from_source(src, Datetime::from_ymd(2000, 1, 1).unwrap());
    let settings = PdfSettings {
        tagged: true,
        pdf_ua: true,
        timestamp: Datetime::from_ymd_hms(2000, 1, 1, 0, 0, 0).unwrap(),
        warnings_as_errors: false,
    };
    let pdf = compile_pdf(&world, &SourceMaps::new(), &settings).unwrap();
    let doc = lopdf::Document::load_mem(&pdf.bytes).unwrap();
    let roles = count_roles(&struct_elems(&doc));
    assert_eq!(roles.get("TH"), None);
    assert_eq!(roles["TD"], 4);
}

#[test]
fn test_table_empty_header_cell_warns() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    std::fs::write(
        dir.path().join("required/03_t.md"),
        "---\ntitle: T\nhero: images/hero.png\n---\n| A |   |\n|---|---|\n| 1 | 2 |\n",
    )
    .unwrap();
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "W006" && w["page"] == "required/03_t.md" && w["line"] == 5),
        "{v}"
    );
}

#[test]
fn test_pdf_has_marked_lang_and_title() {
    let (_out, doc) = full_required();
    let catalog = doc.catalog().unwrap();
    let mark = catalog.get(b"MarkInfo").unwrap().as_dict().unwrap();
    assert!(mark.get(b"Marked").unwrap().as_bool().unwrap());
    let lang = catalog.get(b"Lang").unwrap();
    assert_eq!(String::from_utf8_lossy(lang.as_str().unwrap()), "en");
    let prefs = catalog
        .get(b"ViewerPreferences")
        .unwrap()
        .as_dict()
        .unwrap();
    assert!(prefs.get(b"DisplayDocTitle").unwrap().as_bool().unwrap());
    assert!(catalog.has(b"Outlines"), "headings produce bookmarks");
    // Title in the Info dictionary or in the XMP metadata stream.
    let info_title = doc
        .trailer
        .get(b"Info")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .and_then(|id| doc.get_dictionary(id).ok())
        .and_then(|d| d.get(b"Title").ok())
        .is_some();
    let xmp_title = catalog
        .get(b"Metadata")
        .ok()
        .and_then(|m| m.as_reference().ok())
        .and_then(|id| doc.get_object(id).ok())
        .and_then(|o| o.as_stream().ok())
        .is_some_and(|s| String::from_utf8_lossy(&s.content).contains("The Full Book"));
    assert!(info_title || xmp_title, "document title present");
}

#[test]
fn test_document_language_follows_book_lang() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    let cfg = std::fs::read_to_string(dir.path().join("hideo.toml"))
        .unwrap()
        .replace("lang = \"en\"", "lang = \"fi\"");
    std::fs::write(dir.path().join("hideo.toml"), cfg).unwrap();
    let out = tempfile::tempdir().unwrap();
    let r = hideo()
        .args(["build", "-q", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    assert_eq!(
        doc.catalog()
            .unwrap()
            .get(b"Lang")
            .unwrap()
            .as_str()
            .unwrap(),
        b"fi"
    );
}

#[test]
fn test_decorative_images_are_artifacts_and_figures_have_alt() {
    let (_out, doc) = full_required();
    let mut alts: BTreeSet<String> = BTreeSet::new();
    let mut figures = 0;
    for e in struct_elems(&doc).iter().filter(|e| e.role == "Figure") {
        figures += 1;
        let alt = e
            .dict
            .get(b"Alt")
            .unwrap_or_else(|_| panic!("Figure without Alt: {:?}", e.dict));
        alts.insert(String::from_utf8_lossy(alt.as_str().unwrap()).into_owned());
    }
    // Heroes (decorative), the cover, the contents image, the empty-alt body image and the
    // external-link arrow are artifacts. Real figures: the hero with `hero_alt` and the body image.
    assert_eq!(figures, 2, "{alts:?}");
    assert!(
        alts.contains("A purple to blue gradient"),
        "hero_alt makes the hero content: {alts:?}"
    );
    assert!(
        alts.contains("A colourful gradient used as a figure"),
        "{alts:?}"
    );
}

#[test]
fn test_toc_entry_structure() {
    let (_out, doc) = full_required();
    let roles = count_roles(&struct_elems(&doc));
    assert!(roles["TOC"] >= 1, "contents is a TOC structure: {roles:?}");
    assert!(
        roles["TOCI"] >= 6,
        "contents entries are TOCI structures: {roles:?}"
    );

    let pages = doc.get_pages();
    let catalog = doc.catalog().unwrap();
    let names = match catalog.get(b"Names").unwrap() {
        Object::Reference(id) => doc.get_dictionary(*id).unwrap(),
        Object::Dictionary(dict) => dict,
        _ => unreachable!(),
    };
    let destinations = match names.get(b"Dests").unwrap() {
        Object::Reference(id) => doc.get_dictionary(*id).unwrap(),
        Object::Dictionary(dict) => dict,
        _ => unreachable!(),
    };
    let mut named = Default::default();
    doc.get_named_destinations(destinations, &mut named)
        .unwrap();
    let annotations = doc
        .get_page_annotations(*pages.get(&2).expect("contents page"))
        .unwrap();
    let mut destination_pages = Vec::new();
    for annotation in annotations {
        if annotation.get(b"Subtype").ok().and_then(name_of).as_deref() != Some("Link") {
            continue;
        }
        let destination = annotation.get(b"Dest").expect("TOC link has a destination");
        let destination = match destination {
            Object::Reference(id) => doc.get_object(*id).expect("destination object"),
            object => object,
        };
        let name = destination.as_str().expect("named destination");
        let page = named
            .get(name)
            .unwrap_or_else(|| panic!("missing named destination {name:?}"))
            .page()
            .unwrap()
            .as_reference()
            .expect("destination targets a page");
        assert!(
            pages.values().any(|id| *id == page),
            "TOC destination {page:?} is not a document page"
        );
        destination_pages.push(
            pages
                .iter()
                .find_map(|(number, id)| (*id == page).then_some(*number))
                .unwrap(),
        );
    }

    let expected = [
        "This is the introduction page",
        "A second-level heading",
        "Rust Code",
        "Simple Table",
        "A body image scaled",
        "Unicode headings",
    ]
    .map(|marker| {
        (3..=page_count(&doc) as u32)
            .find(|&number| page_text(&doc, number).contains(marker))
            .unwrap()
    });
    assert_eq!(
        destination_pages, expected,
        "every TOCI link reaches its displayed heading"
    );
}

#[test]
fn test_hero_without_alt_is_artifact() {
    // minimal has two heroes and a cover, and no figures at all.
    let (out, r) = build_fixture("minimal", &["-q"]);
    assert!(r.status.success());
    let doc = load(&out.path().join("required.pdf"));
    let roles = count_roles(&struct_elems(&doc));
    assert_eq!(roles.get("Figure"), None, "{roles:?}");
}

#[test]
fn test_external_link_arrow_is_artifact() {
    let (_out, doc) = full_required();
    let alts: Vec<String> = struct_elems(&doc)
        .iter()
        .filter(|e| e.role == "Figure")
        .filter_map(|e| {
            e.dict.get(b"Alt").ok().and_then(|a| {
                a.as_str()
                    .ok()
                    .map(|b| String::from_utf8_lossy(b).into_owned())
            })
        })
        .collect();
    assert_eq!(
        alts.iter().filter(|a| *a == "external link").count(),
        0,
        "the decorative arrow must not be a Figure: {alts:?}"
    );
}

#[test]
fn test_contents_leader_is_artifact() {
    let (_out, doc) = full_required();
    // Page 2 is the contents. The dotted leaders are drawn as text, so lopdf's text extraction
    // sees them; what matters is that they sit in /Artifact marked content, while titles and
    // page numbers stay tagged content.
    let contents = page_text(&doc, 2);
    assert!(
        contents.contains("Introduction") && contents.contains("Tables"),
        "{contents}"
    );
    let ops = text_ops(&doc, 2);
    assert!(ops.in_artifact > 100, "leader dots are artifacts: {ops:?}");
    assert!(
        ops.in_tagged >= 6,
        "titles and page numbers are tagged: {ops:?}"
    );
    assert_eq!(
        ops.untagged, 0,
        "every text operator is tagged or an artifact: {ops:?}"
    );
    // A normal page has only its footer as artifact text.
    let body = text_ops(&doc, 3);
    assert!(body.in_artifact <= 2, "{body:?}");
}

#[test]
fn test_heading_skipped_level_warns() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    std::fs::write(
        dir.path().join("required/03_skip.md"),
        "---\ntitle: T\nhero: images/hero.png\n---\n# One\n\n### Three\n",
    )
    .unwrap();
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["code"] == "W007" && w["page"] == "required/03_skip.md" && w["line"] == 7),
        "{v}"
    );
}

#[test]
fn test_missing_glyph_is_e006_with_page_and_line() {
    // The embedded fonts do not cover Japanese: PDF/UA-1 turns the missing glyphs into errors
    // that name the page and line. (The "missing alt text" error cannot be provoked, because
    // hideo always supplies alt text or marks the image as an artifact.)
    let (out, r) = build_fixture("edge", &["-q"]);
    assert_eq!(r.status.code(), Some(1));
    let err = stderr(&r);
    assert!(err.contains("error[E006]"), "{err}");
    assert!(err.contains("page: required/01_edge.md"), "{err}");
    assert!(
        err.contains("line: 14"),
        "the Japanese heading is on line 14 of the file: {err}"
    );
    assert!(
        !out.path().join("required.pdf").exists(),
        "no PDF on a validation error"
    );
}

#[test]
fn test_pdf_ua_off_builds_despite_missing_glyphs() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("edge"), dir.path());
    let cfg = std::fs::read_to_string(dir.path().join("hideo.toml")).unwrap();
    std::fs::write(
        dir.path().join("hideo.toml"),
        format!("{cfg}\n[accessibility]\npdf_ua = false\n"),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let r = hideo()
        .args(["build", "-q", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    assert!(out.path().join("required.pdf").exists());
}

/// A copy of the `minimal` fixture whose first page holds a long table: 40 rows, each cell a
/// run of marker words (`r07a1 r07a2 ...`) that wraps to several lines.
fn long_table_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    let mut body = String::from(
        "---\ntitle: \"Long table\"\nhero: \"images/hero.png\"\n---\n\n# Long table\n\n| Name | Details |\n| --- | --- |\n",
    );
    for r in 0..40 {
        let words = |c: char| {
            (1..=40)
                .map(|w| format!("r{r:02}{c}{w}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        body.push_str(&format!("| {} | {} |\n", words('a'), words('b')));
    }
    let required = dir.path().join("required");
    std::fs::write(required.join("01_intro.md"), body).unwrap();
    dir
}

#[test]
fn test_table_rows_are_never_split_across_pages() {
    let dir = long_table_project();
    let out = tempfile::tempdir().unwrap();
    let r = hideo()
        .args(["build", "-q", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    let pages = page_count(&doc) as u32;

    // Which pages hold the marker words of each row.
    let mut row_pages: Vec<std::collections::BTreeSet<u32>> = vec![Default::default(); 40];
    let mut header_pages = std::collections::BTreeSet::new();
    for n in 1..=pages {
        let text = page_text(&doc, n);
        for word in text.split_whitespace() {
            if let Some(rest) = word.strip_prefix('r')
                && rest.len() >= 4
                && let Ok(row) = rest[..2].parse::<usize>()
                && row < 40
            {
                row_pages[row].insert(n);
            }
        }
        if text.contains("Details") {
            header_pages.insert(n);
        }
    }
    let used: std::collections::BTreeSet<u32> = row_pages.iter().flatten().copied().collect();
    assert!(
        used.len() >= 3,
        "the table must cross page boundaries for this test to mean anything: {used:?}"
    );
    for (row, p) in row_pages.iter().enumerate() {
        assert_eq!(p.len(), 1, "row {row} is split over pages {p:?}");
    }
    // The header repeats on every page the table occupies.
    assert!(
        used.is_subset(&header_pages),
        "header on {header_pages:?}, rows on {used:?}"
    );
}

#[test]
fn test_table_row_taller_than_a_page_splits_without_losing_text() {
    // A row that cannot fit on any page must split; an unbreakable row would be clipped and
    // silently drop its text. The rows around it still stay whole.
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    let big = (1..=1500)
        .map(|i| format!("big{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "---\ntitle: \"Huge row\"\nhero: \"images/hero.png\"\n---\n\n# Huge row\n\n| Name | Details |\n| --- | --- |\n| first | row |\n| huge | {big} |\n| last | row |\n"
    );
    std::fs::write(dir.path().join("required/01_intro.md"), body).unwrap();
    let out = tempfile::tempdir().unwrap();
    let r = hideo()
        .args(["build", "-q", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    let text: String = (1..=page_count(&doc) as u32)
        .map(|n| page_text(&doc, n))
        .collect::<Vec<_>>()
        .join(" ");
    let words: std::collections::BTreeSet<&str> = text.split_whitespace().collect();
    for i in 1..=1500 {
        assert!(
            words.contains(format!("big{i}").as_str()),
            "big{i} was lost"
        );
    }
    assert!(
        words.contains("last"),
        "the row after the huge one is present"
    );
}
