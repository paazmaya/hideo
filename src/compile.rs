use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;
use typst::diag::Warned;
use typst::foundations::{Datetime, Smart};
use typst::World;
use typst::WorldExt;
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, PdfStandards, PdfStandard, Timestamp};
use crate::world::HideoWorld;
use crate::config::Config;
use crate::error::{DiagnosticCollector, HideoError, ErrorCode, Warning, WarningCode};
use crate::markdown::emit::LineMap;

pub fn compile_and_export(
    world: HideoWorld,
    line_maps: HashMap<String, LineMap>,
    config: &Config,
    output_path: &Path,
) -> Result<(), Vec<HideoError>> {
    let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);

    let document = output.map_err(|errs| {
        map_diagnostics(&world, &line_maps, &errs)
    })?;

    let standards = if config.accessibility.pdf_ua {
        match PdfStandards::new(&[PdfStandard::Ua_1]) {
            Ok(s) => s,
            Err(e) => {
                return Err(vec![HideoError::new(ErrorCode::E006, format!("PDF/UA-1 validation setup failed: {:?}", e))]);
            }
        }
    } else {
        PdfStandards::default()
    };

    let options = PdfOptions {
        timestamp: Some(Timestamp::new_utc(config.build_timestamp())),
        creator: Smart::Custom(Some(format!("hideo {}", env!("CARGO_PKG_VERSION")))),
        standards,
        tagged: config.accessibility.tagged,
        ..Default::default()
    };

    let bytes = typst_pdf::pdf(&document, &options)
        .map_err(|errs| {
            map_diagnostics(&world, &line_maps, &errs)
        })?;

    write_atomically(output_path, &bytes)?;

    for warning in warnings {
        let mapped = map_typst_warning(&world, &line_maps, &warning);
        if let Some(w) = mapped {
            if config.diagnostics.warnings_as_errors {
                return Err(vec![HideoError::new(ErrorCode::E005, w.message)
                    .with_page(w.page.unwrap_or_default())
                    .with_line(w.line.unwrap_or(0))]);
            }
        }
    }

    Ok(())
}

fn write_atomically(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let temp = NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(temp.path(), bytes)?;
    temp.persist(path)?;
    Ok(())
}

fn map_diagnostics(
    world: &HideoWorld,
    line_maps: &HashMap<String, LineMap>,
    errs: &typst::diag::EcoVec<typst::diag::SourceDiagnostic>,
) -> Vec<HideoError> {
    let mut errors = Vec::new();

    for err in errs {
        let span = err.span;
        let file_id = span.id();

        if let Ok(source) = world.source(file_id) {
            let range = world.range(span).unwrap_or(span.range());
            let byte_start = range.start;
            let line = source.lines().byte_to_line(byte_start).unwrap_or(0);

            let vpath = file_id.vpath().get_without_slash();
            let page_key = if vpath.starts_with("pages/") {
                vpath.strip_prefix("pages/").unwrap().strip_suffix(".typ").unwrap_or(vpath)
            } else if vpath == "main.typ" {
                "main"
            } else {
                vpath
            };

            let line_map = line_maps.get(page_key);
            let markdown_line = line_map
                .and_then(|lm| lm.entries.get(line))
                .copied()
                .unwrap_or(line as u32 + 1) as usize;

            let code = if err.message.to_string().contains("pdf/ua-1") {
                ErrorCode::E006
            } else {
                ErrorCode::E005
            };

            errors.push(HideoError::new(code, err.message.to_string())
                .with_page(page_key.to_string())
                .with_line(markdown_line));
        }
    }

    errors
}

fn map_typst_warning(
    world: &HideoWorld,
    line_maps: &HashMap<String, LineMap>,
    warning: &typst::diag::Warning,
) -> Option<Warning> {
    let span = warning.span;
    let file_id = span.id();

    if let Ok(source) = world.source(file_id) {
        let range = world.range(span).unwrap_or(span.range());
        let byte_start = range.start;
        let line = source.lines().byte_to_line(byte_start).unwrap_or(0);

        let vpath = file_id.vpath().get_without_slash();
        let page_key = if vpath.starts_with("pages/") {
            vpath.strip_prefix("pages/").unwrap().strip_suffix(".typ").unwrap_or(vpath)
        } else if vpath == "main.typ" {
            "main"
        } else {
            vpath
        };

        let line_map = line_maps.get(page_key);
        let markdown_line = line_map
            .and_then(|lm| lm.entries.get(line))
            .copied()
            .unwrap_or(line as u32 + 1) as usize;

        let code = if warning.message.to_string().contains("foreignObject") {
            WarningCode::W004
        } else {
            WarningCode::W001
        };

        Some(Warning::new(code, warning.message.to_string())
            .with_page(page_key.to_string())
            .with_line(markdown_line))
    } else {
        None
    }
}

impl Config {
    pub fn build_timestamp(&self) -> Datetime {
        let epoch = std::env::var("SOURCE_DATE_EPOCH")
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(946684800);

        let dt = chrono::DateTime::from_timestamp(epoch, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(946684800, 0).unwrap());

        Datetime::from_ymd_hms(
            dt.year() as i64,
            dt.month() as u8,
            dt.day() as u8,
            dt.hour() as u8,
            dt.minute() as u8,
            dt.second() as u8,
        ).unwrap()
    }
}