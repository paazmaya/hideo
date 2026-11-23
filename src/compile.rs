//! Compile, export and diagnostic mapping (plan 7.9).

use std::collections::BTreeMap;
use std::path::Path;

use typst::World;
use typst::WorldExt;
use typst::diag::{Severity, SourceDiagnostic, Warned};
use typst::foundations::{Datetime, Smart};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use crate::error::{ConfigError, ErrorCode, HideoError, Warning, WarningCode};
use crate::world::HideoWorld;

/// 2000-01-01T00:00:00Z, used when `SOURCE_DATE_EPOCH` is unset (F19).
pub const DEFAULT_EPOCH: i64 = 946_684_800;

/// Maps lines of one generated `.typ` file back to Markdown.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    /// Markdown file name used in diagnostics (e.g. `required/01_intro.md`).
    pub page: String,
    /// Entry `i` is the Markdown file line that produced generated line `i` (0-based index).
    pub lines: Vec<u32>,
}

/// Source maps keyed by virtual path (`pages/001.typ`).
pub type SourceMaps = BTreeMap<String, SourceMap>;

#[derive(Debug, Clone)]
pub struct PdfSettings {
    pub tagged: bool,
    pub pdf_ua: bool,
    pub timestamp: Datetime,
    pub warnings_as_errors: bool,
    pub suppressed_warnings: Vec<WarningCode>,
    /// Absolute nesting level for each generated page/folder bookmark.
    pub outline_levels: Option<Vec<usize>>,
}

#[derive(Debug)]
pub struct CompiledPdf {
    pub bytes: Vec<u8>,
    pub pages: usize,
    pub warnings: Vec<Warning>,
}

/// `SOURCE_DATE_EPOCH` as a Typst datetime. An invalid value is a usage error (exit 2).
pub fn source_date(value: Option<&str>) -> Result<Datetime, ConfigError> {
    let epoch = match value {
        None => DEFAULT_EPOCH,
        Some(v) => v.trim().parse::<i64>().map_err(|_| {
            ConfigError(format!(
                "SOURCE_DATE_EPOCH must be an integer number of seconds, got `{v}`"
            ))
        })?,
    };
    datetime_from_epoch(epoch)
        .ok_or_else(|| ConfigError(format!("SOURCE_DATE_EPOCH `{epoch}` is out of range")))
}

pub fn datetime_from_epoch(epoch: i64) -> Option<Datetime> {
    let days = epoch.div_euclid(86_400);
    let secs = epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Datetime::from_ymd_hms(
        i32::try_from(year).ok()?,
        month,
        day,
        (secs / 3_600) as u8,
        (secs % 3_600 / 60) as u8,
        (secs % 60) as u8,
    )
}

/// Proleptic Gregorian date for a count of days since 1970-01-01 (Howard Hinnant's
/// `civil_from_days`). Written out here so that no date-time crate is needed.
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Compile to a paged document without exporting. Used by tests and by `compile_pdf`.
pub fn compile_document(world: &HideoWorld) -> Warned<typst::diag::SourceResult<PagedDocument>> {
    typst::compile::<PagedDocument>(world)
}

/// Compile and export a PDF. Nothing is written to disk.
pub fn compile_pdf(
    world: &HideoWorld,
    maps: &SourceMaps,
    settings: &PdfSettings,
) -> Result<CompiledPdf, Vec<HideoError>> {
    let Warned { output, warnings } = compile_document(world);
    let mut mapped_warnings: Vec<Warning> = warnings
        .iter()
        .map(|w| map_warning(world, maps, w))
        .collect();
    mapped_warnings.retain(|w| !settings.suppressed_warnings.contains(&w.code));

    let document = output.map_err(|errs| map_errors(world, maps, &errs, ErrorCode::E005))?;

    let standards = if settings.pdf_ua {
        PdfStandards::new(&[PdfStandard::Ua_1]).map_err(|e| {
            vec![HideoError::new(
                ErrorCode::E006,
                format!("cannot enable PDF/UA-1: {e:?}"),
            )]
        })?
    } else {
        PdfStandards::default()
    };
    let options = PdfOptions {
        timestamp: Some(Timestamp::new_utc(settings.timestamp)),
        creator: Smart::Custom(Some(format!("hideo {}", env!("CARGO_PKG_VERSION")))),
        standards,
        tagged: settings.tagged,
        ..Default::default()
    };
    let mut bytes = typst_pdf::pdf(&document, &options)
        .map_err(|errs| map_errors(world, maps, &errs, ErrorCode::E006))?;
    if let Some(levels) = &settings.outline_levels {
        bytes = normalize_outline(bytes, levels)?;
    }

    mapped_warnings
        .sort_by(|a, b| (&a.page, a.line, &a.message).cmp(&(&b.page, b.line, &b.message)));
    mapped_warnings.dedup_by(|a, b| a.page == b.page && a.line == b.line && a.message == b.message);

    if settings.warnings_as_errors && !mapped_warnings.is_empty() {
        return Err(mapped_warnings
            .iter()
            .map(|w| {
                let mut e = HideoError::new(ErrorCode::E005, w.message.clone());
                e.page = w.page.clone();
                e.line = w.line;
                e
            })
            .collect());
    }

    fn normalize_outline(bytes: Vec<u8>, levels: &[usize]) -> Result<Vec<u8>, Vec<HideoError>> {
        let mut document = lopdf::Document::load_mem(&bytes).map_err(|e| {
            vec![HideoError::new(
                ErrorCode::E006,
                format!("cannot read PDF outline: {e}"),
            )]
        })?;
        let catalog = document.catalog().map_err(|e| {
            vec![HideoError::new(
                ErrorCode::E006,
                format!("cannot read PDF catalog: {e}"),
            )]
        })?;
        let Some(outlines_id) = catalog
            .get(b"Outlines")
            .ok()
            .and_then(|object| object.as_reference().ok())
        else {
            return Ok(bytes);
        };
        let first = document
            .get_dictionary(outlines_id)
            .ok()
            .and_then(|outlines| outlines.get(b"First").ok().cloned());
        let Some(first) = first else {
            return Ok(bytes);
        };

        let mut nodes = Vec::new();
        flatten_outline(&document, first, &mut nodes);
        if nodes.len() != levels.len() {
            return Err(vec![HideoError::new(
                ErrorCode::E006,
                format!(
                    "cannot normalize PDF outline: expected {} bookmarks, found {}",
                    levels.len(),
                    nodes.len()
                ),
            )]);
        }
        for level in levels {
            if *level == 0 {
                return Err(vec![HideoError::new(
                    ErrorCode::E006,
                    "cannot normalize PDF outline: bookmark level is zero",
                )]);
            }
        }

        const CHILDREN_KEYS: [&[u8]; 3] = [b"Parent", b"Prev", b"Next"];
        for node in &nodes {
            let dict = document.get_dictionary_mut(*node).unwrap();
            for key in CHILDREN_KEYS {
                dict.remove(key);
            }
            dict.remove(b"First");
            dict.remove(b"Last");
            dict.remove(b"Count");
        }
        document
            .get_dictionary_mut(outlines_id)
            .unwrap()
            .remove(b"First");
        document
            .get_dictionary_mut(outlines_id)
            .unwrap()
            .remove(b"Last");

        let mut stack: Vec<lopdf::ObjectId> = Vec::new();
        for (&node, &level) in nodes.iter().zip(levels) {
            stack.truncate(level.saturating_sub(1));
            if level == 1 {
                append_outline_child(&mut document, outlines_id, node);
            } else {
                let Some(parent) = stack.last().copied() else {
                    return Err(vec![HideoError::new(
                        ErrorCode::E006,
                        format!("cannot normalize PDF outline: missing parent for level {level}"),
                    )]);
                };
                append_outline_child(&mut document, parent, node);
            }
            stack.push(node);
        }

        let mut output = Vec::new();
        document.save_to(&mut output).map_err(|e| {
            vec![HideoError::new(
                ErrorCode::E006,
                format!("cannot save normalized PDF outline: {e}"),
            )]
        })?;
        Ok(output)
    }

    fn flatten_outline(
        document: &lopdf::Document,
        object: lopdf::Object,
        output: &mut Vec<lopdf::ObjectId>,
    ) {
        let Some(id) = object.as_reference().ok() else {
            return;
        };
        output.push(id);
        let Ok(dictionary) = document.get_dictionary(id) else {
            return;
        };
        if let Ok(first) = dictionary.get(b"First") {
            flatten_outline(document, first.clone(), output);
        }
        if let Ok(next) = dictionary.get(b"Next") {
            flatten_outline(document, next.clone(), output);
        }
    }

    fn append_outline_child(
        document: &mut lopdf::Document,
        parent: lopdf::ObjectId,
        child: lopdf::ObjectId,
    ) {
        let previous = document
            .get_dictionary(parent)
            .ok()
            .and_then(|dict| dict.get(b"Last").ok())
            .and_then(|object| object.as_reference().ok());
        document
            .get_dictionary_mut(parent)
            .unwrap()
            .set("Count", 0_i64);
        if let Some(previous) = previous {
            document
                .get_dictionary_mut(previous)
                .unwrap()
                .set("Next", child);
            document
                .get_dictionary_mut(child)
                .unwrap()
                .set("Prev", previous);
        } else {
            document
                .get_dictionary_mut(parent)
                .unwrap()
                .set("First", child);
        }
        document
            .get_dictionary_mut(parent)
            .unwrap()
            .set("Last", child);
        document
            .get_dictionary_mut(child)
            .unwrap()
            .set("Parent", parent);
    }

    Ok(CompiledPdf {
        bytes,
        pages: document.pages().len(),
        warnings: mapped_warnings,
    })
}

/// Write `bytes` to `path` via a temporary file in the same directory, then rename, so an error
/// never leaves a half-written PDF.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

struct Location {
    page: Option<String>,
    line: Option<usize>,
    /// The diagnostic is in generated code that is not a page (main.typ, the template).
    internal: bool,
}

fn locate(world: &HideoWorld, maps: &SourceMaps, d: &SourceDiagnostic) -> Location {
    let Some(id) = d.span.id() else {
        return Location {
            page: None,
            line: None,
            internal: false,
        };
    };
    let vpath = id.vpath().get_without_slash().to_string();
    let line_index = world.range(d.span).and_then(|range| {
        world
            .source(id)
            .ok()
            .and_then(|src| src.lines().byte_to_line(range.start))
    });
    match maps.get(&vpath) {
        Some(map) => Location {
            page: Some(map.page.clone()),
            line: line_index
                .and_then(|i| map.lines.get(i))
                .map(|l| *l as usize),
            internal: false,
        },
        None => Location {
            page: Some(vpath),
            line: line_index.map(|i| i + 1),
            internal: true,
        },
    }
}

fn map_errors(
    world: &HideoWorld,
    maps: &SourceMaps,
    errs: &[SourceDiagnostic],
    code: ErrorCode,
) -> Vec<HideoError> {
    errs.iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| {
            let loc = locate(world, maps, d);
            let mut message = d.message.to_string();
            for hint in &d.hints {
                message.push_str(&format!("\n  hint: {}", hint.v));
            }
            if loc.internal {
                message.push_str("\n  this is a hideo bug, please report it");
            }
            let mut e = HideoError::new(code, message);
            e.page = loc.page;
            e.line = loc.line;
            e
        })
        .collect()
}

fn map_warning(world: &HideoWorld, maps: &SourceMaps, d: &SourceDiagnostic) -> Warning {
    let loc = locate(world, maps, d);
    let mut w = Warning::new(WarningCode::W010, format!("typst: {}", d.message));
    w.page = loc.page;
    w.line = loc.line;
    w
}

/// Text of every page in reading order, for tests that compare extracted text.
pub fn extract_text(document: &PagedDocument) -> Vec<String> {
    use typst::layout::{Frame, FrameItem};
    fn walk(frame: &Frame, out: &mut String) {
        for (_, item) in frame.items() {
            match item {
                FrameItem::Text(t) => out.push_str(&t.text),
                FrameItem::Group(g) => walk(&g.frame, out),
                _ => {}
            }
        }
    }
    document
        .pages()
        .iter()
        .map(|p| {
            let mut s = String::new();
            walk(&p.frame, &mut s);
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_from_source_date_epoch() {
        let d = source_date(Some("1700000000")).unwrap();
        assert_eq!(d, Datetime::from_ymd_hms(2023, 11, 14, 22, 13, 20).unwrap());
        let default = source_date(None).unwrap();
        assert_eq!(
            default,
            Datetime::from_ymd_hms(2000, 1, 1, 0, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_invalid_source_date_epoch_exits_2() {
        for bad in [
            "",
            "abc",
            "1.5",
            "99999999999999999999",
            &i64::MAX.to_string(),
        ] {
            assert!(source_date(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn test_map_warning() {
        let mut files = BTreeMap::new();
        files.insert("main.typ".into(), b"#set page(paper: \"a4\")".to_vec());
        let dt = Datetime::from_ymd(2025, 1, 1).unwrap();
        let world = HideoWorld::new(&files, dt).unwrap();
        let maps = SourceMaps::new();
        let diag = SourceDiagnostic::warning(typst::syntax::Span::detached(), "test warning");
        let w = map_warning(&world, &maps, &diag);
        assert_eq!(w.code, WarningCode::W010);
        assert!(w.message.contains("test warning"));
    }

    #[test]
    fn test_compile_pdf_with_typst_warning() {
        let mut files = BTreeMap::new();
        files.insert(
            "main.typ".into(),
            b"#set page(paper: \"a4\")\n#set document(title: \"T\")\n#show page: it => it\n#show par: set block(spacing: 10pt)\nHello\n".to_vec(),
        );
        let dt = Datetime::from_ymd(2025, 1, 1).unwrap();
        let world = HideoWorld::new(&files, dt).unwrap();
        let maps = SourceMaps::new();

        let settings = PdfSettings {
            tagged: false,
            pdf_ua: false,
            timestamp: dt,
            warnings_as_errors: false,
            suppressed_warnings: Vec::new(),
            outline_levels: None,
        };
        let res = compile_pdf(&world, &maps, &settings).unwrap();
        assert_eq!(res.warnings.len(), 2);

        let settings_err = PdfSettings {
            tagged: false,
            pdf_ua: false,
            timestamp: dt,
            warnings_as_errors: true,
            suppressed_warnings: Vec::new(),
            outline_levels: None,
        };
        let res_err = compile_pdf(&world, &maps, &settings_err);
        assert!(res_err.is_err());

        let settings_suppressed = PdfSettings {
            tagged: false,
            pdf_ua: false,
            timestamp: dt,
            warnings_as_errors: true,
            suppressed_warnings: vec![WarningCode::W010],
            outline_levels: None,
        };
        let res_suppressed = compile_pdf(&world, &maps, &settings_suppressed).unwrap();
        assert!(res_suppressed.warnings.is_empty());
    }

    #[test]
    fn test_epoch_to_date_covers_leap_days_and_negative_epochs() {
        let cases = [
            (0, (1970, 1, 1, 0, 0, 0)),
            (951_782_400, (2000, 2, 29, 0, 0, 0)),
            (951_868_800, (2000, 3, 1, 0, 0, 0)),
            (4_107_542_399, (2100, 2, 28, 23, 59, 59)),
            (-1, (1969, 12, 31, 23, 59, 59)),
            (-2_208_988_800, (1900, 1, 1, 0, 0, 0)),
        ];
        for (epoch, (y, mo, d, h, mi, s)) in cases {
            assert_eq!(
                datetime_from_epoch(epoch),
                Datetime::from_ymd_hms(y, mo, d, h, mi, s),
                "{epoch}"
            );
        }
        assert!(datetime_from_epoch(i64::MAX).is_none());
    }

    fn world(src: &str) -> HideoWorld {
        HideoWorld::from_source(src, Datetime::from_ymd(2000, 1, 1).unwrap())
    }

    fn settings(pdf_ua: bool) -> PdfSettings {
        PdfSettings {
            tagged: true,
            pdf_ua,
            timestamp: Datetime::from_ymd_hms(2000, 1, 1, 0, 0, 0).unwrap(),
            warnings_as_errors: false,
            suppressed_warnings: Vec::new(),
            outline_levels: None,
        }
    }

    /// The day-1 vertical slice (plan Appendix E): hard-coded Typst -> PDF -> lopdf reads it.
    #[test]
    fn vertical_slice_compiles_and_lopdf_reads_one_page() {
        let w = world("#set document(title: \"T\")\n#set text(lang: \"en\")\nHello *world*.");
        let pdf = compile_pdf(&w, &SourceMaps::new(), &settings(true)).unwrap();
        assert_eq!(pdf.pages, 1);
        let doc = lopdf::Document::load_mem(&pdf.bytes).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
        assert!(pdf.bytes.starts_with(b"%PDF-"));
    }

    #[test]
    fn pdf_ua_missing_title_is_an_error_e006() {
        let w = world("#set text(lang: \"en\")\nNo title.");
        let errors = compile_pdf(&w, &SourceMaps::new(), &settings(true)).unwrap_err();
        assert!(
            errors.iter().all(|e| e.code == ErrorCode::E006),
            "{errors:?}"
        );
        // The same document passes with validation off.
        assert!(compile_pdf(&w, &SourceMaps::new(), &settings(false)).is_ok());
    }

    #[test]
    fn typst_error_in_mapped_file_names_page_and_markdown_line() {
        let mut files = BTreeMap::new();
        files.insert(
            "main.typ".to_string(),
            b"#include \"pages/001.typ\"".to_vec(),
        );
        files.insert(
            "pages/001.typ".to_string(),
            b"fine\n\n#undefined-function()\n".to_vec(),
        );
        let w = HideoWorld::new(&files, Datetime::from_ymd(2000, 1, 1).unwrap()).unwrap();
        let mut maps = SourceMaps::new();
        maps.insert(
            "pages/001.typ".to_string(),
            SourceMap {
                page: "required/01_intro.md".into(),
                lines: vec![10, 11, 42],
            },
        );
        let errors = compile_pdf(&w, &maps, &settings(false)).unwrap_err();
        assert_eq!(errors[0].code, ErrorCode::E005);
        assert_eq!(errors[0].page.as_deref(), Some("required/01_intro.md"));
        assert_eq!(errors[0].line, Some(42));
    }

    #[test]
    fn internal_errors_say_so() {
        let w = world("#undefined-function()");
        let errors = compile_pdf(&w, &SourceMaps::new(), &settings(false)).unwrap_err();
        assert!(errors[0].message.contains("hideo bug"));
        assert_eq!(errors[0].page.as_deref(), Some("main.typ"));
    }

    #[test]
    fn no_partial_pdf_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.pdf");
        let w = world("#undefined()");
        if let Ok(pdf) = compile_pdf(&w, &SourceMaps::new(), &settings(false)) {
            write_atomically(&target, &pdf.bytes).unwrap();
        }
        assert!(!target.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn write_atomically_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sub/out.pdf");
        write_atomically(&target, b"one").unwrap();
        write_atomically(&target, b"two").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"two");
        assert_eq!(
            std::fs::read_dir(target.parent().unwrap()).unwrap().count(),
            1
        );
    }

    #[test]
    fn reproducible_in_process() {
        let src = "#set document(title: \"T\")\n#set text(lang: \"en\")\nSame bytes.";
        let a = compile_pdf(&world(src), &SourceMaps::new(), &settings(true)).unwrap();
        let b = compile_pdf(&world(src), &SourceMaps::new(), &settings(true)).unwrap();
        assert_eq!(a.bytes, b.bytes);
    }

    #[test]
    fn extract_text_reads_frames() {
        let w = world("Hello there");
        let Warned { output, .. } = compile_document(&w);
        let text = extract_text(&output.unwrap());
        assert_eq!(text, vec!["Hello there".to_string()]);
    }
}
