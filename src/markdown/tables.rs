use crate::markdown::escape::escape_text;
use crate::markdown::escape::Position;

#[derive(Debug, Clone)]
pub struct TableColumn {
    pub alignment: Option<TableAlignment>,
    pub min_width: f32,
    pub pref_width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Left,
    Center,
    Right,
}

impl Default for TableAlignment {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub columns: Vec<TableColumn>,
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

pub fn process_table(
    alignments: &[pulldown_cmark::Alignment],
    header_cells: &[Vec<pulldown_cmark::Event>],
    body_rows: &[Vec<Vec<pulldown_cmark::Event>]>,
    text_width_pt: f32,
    font_size_pt: f32,
    diagnostics: &mut crate::error::DiagnosticCollector,
    page_key: &str,
    line: usize,
) -> String {
    let num_cols = alignments.len().max(header_cells.len());

    let mut columns = Vec::with_capacity(num_cols);
    for i in 0..num_cols {
        let alignment = alignments.get(i).copied().map(convert_alignment);
        columns.push(TableColumn {
            alignment,
            min_width: 0.0,
            pref_width: 0.0,
        });
    }

    let header = header_cells.iter()
        .map(|cell| extract_cell_text(cell))
        .collect::<Vec<_>>();

    if header.iter().any(|h| h.trim().is_empty()) {
        diagnostics.add_warning(
            crate::error::Warning::new(crate::error::WarningCode::W006, "empty table header cell")
                .with_page(page_key.to_string())
                .with_line(line),
        );
    }

    let rows: Vec<Vec<String>> = body_rows.iter()
        .map(|row| row.iter().map(extract_cell_text).collect())
        .collect();

    let mut table = Table {
        columns,
        header,
        rows,
    };

    measure_table(&mut table, font_size_pt);
    let fractions = compute_fractions(&table, text_width_pt);
    let font_size = select_font_size(&table, text_width_pt, font_size_pt, diagnostics, page_key, line);

    emit_table(&table, &fractions, font_size)
}

fn convert_alignment(align: pulldown_cmark::Alignment) -> Option<TableAlignment> {
    match align {
        pulldown_cmark::Alignment::Left => Some(TableAlignment::Left),
        pulldown_cmark::Alignment::Center => Some(TableAlignment::Center),
        pulldown_cmark::Alignment::Right => Some(TableAlignment::Right),
        pulldown_cmark::Alignment::None => None,
    }
}

fn extract_cell_text(events: &[pulldown_cmark::Event]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                text.push_str(t.as_ref());
            }
            pulldown_cmark::Event::SoftBreak | pulldown_cmark::Event::HardBreak => {
                text.push(' ');
            }
            _ => {}
        }
    }
    break_long_strings(&text)
}

fn break_long_strings(text: &str) -> String {
    const THRESHOLD: usize = 16;
    let mut result = String::with_capacity(text.len() + 8);
    let mut token = String::new();

    for c in text.chars() {
        if c.is_whitespace() {
            if token.len() >= THRESHOLD {
                result.push_str(&insert_zwsp(&token));
            } else {
                result.push_str(&token);
            }
            token.clear();
            result.push(c);
        } else {
            token.push(c);
        }
    }

    if token.len() >= THRESHOLD {
        result.push_str(&insert_zwsp(&token));
    } else {
        result.push_str(&token);
    }

    result
}

fn insert_zwsp(token: &str) -> String {
    const BREAK_CHARS: &[char] = &['.', '/', '_', '-', '?', '=', '&'];
    let mut result = String::with_capacity(token.len() + 4);
    let mut last_break = 0;

    for (i, c) in token.char_indices() {
        result.push(c);
        if BREAK_CHARS.contains(&c) && i - last_break >= 4 {
            result.push('\u{200B}');
            last_break = i + 1;
        }
    }

    result
}

fn measure_table(table: &mut Table, font_size_pt: f32) {
    let avg_glyph_width = font_size_pt * 0.5;

    for (col_idx, col) in table.columns.iter_mut().enumerate() {
        let header_text = table.header.get(col_idx).map(|h| h.as_str()).unwrap_or("");
        col.pref_width = header_text.len() as f32 * avg_glyph_width;
        col.min_width = col.pref_width * 0.6;

        for row in &table.rows {
            if let Some(cell) = row.get(col_idx) {
                let cell_width = cell.len() as f32 * avg_glyph_width;
                col.pref_width = col.pref_width.max(cell_width);
                col.min_width = col.min_width.min(cell_width);
            }
        }
    }
}

fn compute_fractions(table: &Table, text_width_pt: f32) -> Vec<f32> {
    let total_pref: f32 = table.columns.iter().map(|c| c.pref_width).sum();
    if total_pref <= text_width_pt {
        table.columns.iter().map(|c| c.pref_width / total_pref).collect()
    } else {
        let total_min: f32 = table.columns.iter().map(|c| c.min_width).sum();
        if total_min >= text_width_pt {
            table.columns.iter().map(|c| c.min_width / total_min).collect()
        } else {
            let mut fractions: Vec<f32> = table.columns.iter().map(|c| c.min_width).collect();
            let remaining = text_width_pt - total_min;
            let extra_total: f32 = table.columns.iter().map(|c| c.pref_width - c.min_width).sum();
            for (i, col) in table.columns.iter().enumerate() {
                let extra = (col.pref_width - col.min_width) / extra_total * remaining;
                fractions[i] += extra;
            }
            fractions
        }
    }
}

fn select_font_size(
    table: &Table,
    text_width_pt: f32,
    base_font_size: f32,
    diagnostics: &mut crate::error::DiagnosticCollector,
    page_key: &str,
    line: usize,
) -> f32 {
    for &size in &[8.5, 7.5, 7.0] {
        let _avg_glyph_width = size * 0.5;
        let total_min: f32 = table.columns.iter().map(|c| c.min_width * (size / base_font_size)).sum();
        if total_min <= text_width_pt {
            return size;
        }
    }
    diagnostics.add_warning(
        crate::error::Warning::new(crate::error::WarningCode::W004, "table overflows page width, using minimum font size")
            .with_page(page_key.to_string())
            .with_line(line),
    );
    7.0
}

fn emit_table(table: &Table, fractions: &[f32], font_size: f32) -> String {
    let mut out = String::new();

    let cols_spec = fractions.iter()
        .map(|f| format!("{}fr", f))
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str(&format!("#set text(size: {}pt)\n", font_size));
    out.push_str(&format!("#table(\n  columns: ({}),\n", cols_spec));

    out.push_str("  table.header(\n");
    for (i, cell) in table.header.iter().enumerate() {
        let align = table.columns.get(i).and_then(|c| c.alignment).unwrap_or(TableAlignment::Left);
        let align_str = match align {
            TableAlignment::Left => "left",
            TableAlignment::Center => "center",
            TableAlignment::Right => "right",
        };
        let escaped = escape_text(cell, Position::Inline);
        out.push_str(&format!("    align({})[{}],\n", align_str, escaped));
    }
    out.push_str("  ),\n");

    for (row_idx, row) in table.rows.iter().enumerate() {
        if row_idx % 2 == 1 {
            out.push_str("  fill: rgb(\"F5F3F9\"),\n");
        }
        for (i, cell) in row.iter().enumerate() {
            let align = table.columns.get(i).and_then(|c| c.alignment).unwrap_or(TableAlignment::Left);
            let align_str = match align {
                TableAlignment::Left => "left",
                TableAlignment::Center => "center",
                TableAlignment::Right => "right",
            };
            let escaped = escape_text(cell, Position::Inline);
            out.push_str(&format!("    align({})[{}],\n", align_str, escaped));
        }
        if row_idx < table.rows.len() - 1 {
            out.push_str(",\n");
        }
    }

    out.push_str(")\n");
    out
}

use crate::markdown::escape::Position;