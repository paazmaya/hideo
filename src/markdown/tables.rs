//! Table layout and emission (plan F16, 7.5 "Tables").
//!
//! The header row is always emitted as `table.header(...)`. A plain first row would be tagged
//! `TD` in the PDF whatever its styling; only cells inside `table.header` become `TH` with
//! column scope (plan 7.10).

use pulldown_cmark::Alignment;

/// Font sizes tried in order; the first where all minimum column widths fit wins.
pub const FONT_SIZES_PT: [f64; 3] = [8.5, 7.5, 7.0];
/// Horizontal cell inset on each side (Appendix B: 4 pt).
pub const CELL_INSET_PT: f64 = 4.0;
/// Average glyph width of Libertinus Serif relative to the font size (plan: 0.5 em,
/// approximate; the hand-computed unit tests below pin the algorithm, not the constant).
pub const AVG_GLYPH_EM: f64 = 0.5;
/// Tokens at least this long get break opportunities.
pub const LONG_TOKEN_CHARS: usize = 16;
pub const ZWSP: char = '\u{200B}';

#[derive(Debug, Clone, Default)]
pub struct Cell {
    /// Ready to paste between `[` and `]`.
    pub markup: String,
    /// Plain text, used only to measure widths.
    pub plain: String,
}

#[derive(Debug, Clone, Default)]
pub struct TableRow {
    pub cells: Vec<Cell>,
    /// Markdown file line.
    pub line: usize,
}

#[derive(Debug, Clone, Default)]
pub struct TableData {
    pub aligns: Vec<Alignment>,
    pub header: TableRow,
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub font_size_pt: f64,
    /// Column widths as fractions summing to 1.0.
    pub fractions: Vec<f64>,
    /// Even the smallest font does not fit the minimum widths (warning W004).
    pub overflow: bool,
}

/// Insert U+200B after `.`, `/`, `_`, `-`, `?`, `=`, `&` in whitespace-delimited tokens of at
/// least 16 characters, so long URLs and identifiers can wrap. Applied **before** escaping; the
/// ZWSP itself is never escaped. The last character of a token never gets a break after it.
pub fn break_long_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut token = String::new();
    let flush = |token: &mut String, out: &mut String| {
        if token.chars().count() >= LONG_TOKEN_CHARS {
            let chars: Vec<char> = token.chars().collect();
            for (i, c) in chars.iter().enumerate() {
                out.push(*c);
                if matches!(c, '.' | '/' | '_' | '-' | '?' | '=' | '&') && i + 1 < chars.len() {
                    out.push(ZWSP);
                }
            }
        } else {
            out.push_str(token);
        }
        token.clear();
    };
    for c in text.chars() {
        if c.is_whitespace() {
            flush(&mut token, &mut out);
            out.push(c);
        } else {
            token.push(c);
        }
    }
    flush(&mut token, &mut out);
    out
}

fn width(chars: usize, size_pt: f64) -> f64 {
    chars as f64 * AVG_GLYPH_EM * size_pt
}

fn longest_token_chars(text: &str) -> usize {
    text.split(|c: char| c.is_whitespace() || c == ZWSP)
        .map(|t| t.chars().count())
        .max()
        .unwrap_or(0)
}

fn column_count(data: &TableData) -> usize {
    data.rows
        .iter()
        .map(|r| r.cells.len())
        .chain([data.header.cells.len(), data.aligns.len()])
        .max()
        .unwrap_or(0)
}

/// (preferred, minimum) width per column at `size_pt`, including cell insets.
fn column_widths(data: &TableData, size_pt: f64) -> Vec<(f64, f64)> {
    let cols = column_count(data);
    let mut result = vec![(0.0f64, 0.0f64); cols];
    for row in std::iter::once(&data.header).chain(data.rows.iter()) {
        for (i, cell) in row.cells.iter().enumerate() {
            let pref = width(cell.plain.chars().count(), size_pt);
            let min = width(longest_token_chars(&cell.plain), size_pt);
            result[i].0 = result[i].0.max(pref);
            result[i].1 = result[i].1.max(min);
        }
    }
    for w in &mut result {
        w.0 += 2.0 * CELL_INSET_PT;
        w.1 += 2.0 * CELL_INSET_PT;
    }
    result
}

/// Choose the font size and column fractions for `text_width_pt`.
pub fn plan(data: &TableData, text_width_pt: f64) -> Layout {
    let mut chosen = None;
    for size in FONT_SIZES_PT {
        let widths = column_widths(data, size);
        let sum_min: f64 = widths.iter().map(|w| w.1).sum();
        if sum_min <= text_width_pt {
            chosen = Some(size);
            break;
        }
    }
    let overflow = chosen.is_none();
    let font_size_pt = chosen.unwrap_or(*FONT_SIZES_PT.last().unwrap());
    let widths = column_widths(data, font_size_pt);
    let sum_pref: f64 = widths.iter().map(|w| w.0).sum();
    let sum_min: f64 = widths.iter().map(|w| w.1).sum();

    let target: Vec<f64> = if sum_pref <= text_width_pt {
        widths.iter().map(|w| w.0).collect()
    } else if sum_min >= text_width_pt || sum_pref <= sum_min {
        widths.iter().map(|w| w.1).collect()
    } else {
        let spare = (text_width_pt - sum_min) / (sum_pref - sum_min);
        widths.iter().map(|w| w.1 + (w.0 - w.1) * spare).collect()
    };
    let total: f64 = target.iter().sum();
    let fractions = if total > 0.0 {
        target.iter().map(|w| w / total).collect()
    } else {
        vec![1.0 / target.len().max(1) as f64; target.len()]
    };
    Layout {
        font_size_pt,
        fractions,
        overflow,
    }
}

fn align_name(a: Option<&Alignment>) -> &'static str {
    match a {
        Some(Alignment::Center) => "center",
        Some(Alignment::Right) => "right",
        _ => "left",
    }
}

/// Typst lines for the table with the Markdown line each came from.
pub fn emit(data: &TableData, layout: &Layout) -> Vec<(String, usize)> {
    let cols = layout.fractions.len();
    let mut lines: Vec<(String, usize)> = Vec::new();
    let first = data.header.line;
    lines.push(("#[".to_string(), first));
    lines.push((format!("#set text(size: {}pt)", layout.font_size_pt), first));
    lines.push(("#table(".to_string(), first));
    let columns = layout
        .fractions
        .iter()
        .map(|f| format!("{f:.4}fr"))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push((format!("  columns: ({columns}),"), first));
    let aligns = (0..cols)
        .map(|i| align_name(data.aligns.get(i)))
        .collect::<Vec<_>>()
        .join(", ");
    // A one-element array needs a trailing comma in Typst.
    lines.push((format!("  align: ({aligns},),"), first));

    let row_line = |cells: &[Cell], wrap: bool| -> String {
        let mut parts: Vec<String> = cells.iter().map(|c| format!("[{}]", c.markup)).collect();
        while parts.len() < cols {
            parts.push("[]".to_string());
        }
        let joined = parts.join(", ");
        if wrap {
            format!("  table.header({joined}),")
        } else {
            format!("  {joined},")
        }
    };
    lines.push((row_line(&data.header.cells, true), data.header.line));
    for row in &data.rows {
        lines.push((row_line(&row.cells, false), row.line));
    }
    lines.push((")".to_string(), first));
    lines.push(("]".to_string(), first));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(text: &str) -> Cell {
        Cell {
            markup: text.to_string(),
            plain: text.to_string(),
        }
    }

    fn data(header: &[&str], rows: &[&[&str]]) -> TableData {
        TableData {
            aligns: vec![Alignment::None; header.len()],
            header: TableRow {
                cells: header.iter().map(|t| cell(t)).collect(),
                line: 10,
            },
            rows: rows
                .iter()
                .enumerate()
                .map(|(i, r)| TableRow {
                    cells: r.iter().map(|t| cell(t)).collect(),
                    line: 12 + i,
                })
                .collect(),
        }
    }

    const W: f64 = 453.5; // 170 mm text width in pt

    #[test]
    fn test_table_narrow_one_size() {
        // Hand computed at 8.5 pt: glyph = 4.25 pt, inset 8 pt.
        // col0: "ab" pref 2*4.25+8 = 16.5, col1: "abcdef" pref 6*4.25+8 = 33.5; sum 50 <= W.
        let d = data(&["ab", "abcdef"], &[&["a", "b"]]);
        let l = plan(&d, W);
        assert_eq!(l.font_size_pt, 8.5);
        assert!(!l.overflow);
        assert!(
            (l.fractions[0] - 16.5 / 50.0).abs() < 1e-9,
            "{:?}",
            l.fractions
        );
        assert!((l.fractions[1] - 33.5 / 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_table_wide_reduced_font() {
        // Eight columns each with a 16-char unbreakable word. At 8.5 pt min = 16*4.25+8 = 76,
        // sum 608 > 453.5. At 7.5 pt: 16*3.75+8 = 68, sum 544 > 453.5. At 7.0 pt: 16*3.5+8 = 64,
        // sum 512 > 453.5, so nothing fits -> 7.0 pt and overflow.
        let word = "abcdefghijklmnop";
        let d = data(&[word; 8], &[&[word; 8]]);
        let l = plan(&d, W);
        assert_eq!(l.font_size_pt, 7.0);
        assert!(l.overflow);

        // Six columns: 8.5pt: 6*76 = 456 > 453.5; 7.5pt: 6*68 = 408 <= 453.5 -> 7.5.
        let d = data(&[word; 6], &[&[word; 6]]);
        let l = plan(&d, W);
        assert_eq!(l.font_size_pt, 7.5);
        assert!(!l.overflow);
    }

    #[test]
    fn test_table_mixed_widths_distribute_spare_space() {
        // col0 short, col1 long text with spaces: preferred > available, minimum small.
        let long = "word ".repeat(60);
        let d = data(&["id", "text"], &[&["1", long.trim()]]);
        let l = plan(&d, W);
        let s = l.font_size_pt;
        assert_eq!(s, 8.5);
        let min0 = 2.0 * 4.25 + 8.0; // "id"
        assert!((l.fractions.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(
            l.fractions[1] > 0.8,
            "long column gets most of the width: {:?}",
            l.fractions
        );
        assert!(
            l.fractions[0] >= min0 / W - 1e-9,
            "short column keeps its minimum"
        );
    }

    #[test]
    fn test_table_long_word_breaks() {
        let t = "https://example.com/a/very/long/path?x=1&y=2";
        let broken = break_long_tokens(t);
        assert!(broken.contains(ZWSP));
        assert_eq!(broken.replace(ZWSP, ""), t);
        // after '/', '.', '?', '=', '&'
        assert!(broken.contains(&format!("/{ZWSP}example.{ZWSP}com")));
        // short tokens are untouched
        assert_eq!(break_long_tokens("a.b/c short"), "a.b/c short");
        // exactly 15 chars: untouched; 16 chars: broken
        assert_eq!(break_long_tokens("a.b.c.d.e.f.g.h"), "a.b.c.d.e.f.g.h");
        // the last character of a token never gets a break after it
        assert!(!break_long_tokens("a.b.c.d.e.f.g.h.").ends_with(ZWSP));
        assert!(break_long_tokens("a.b.c.d.e.f.g.hi").contains(ZWSP));
        // whitespace is preserved
        assert_eq!(break_long_tokens("x  y"), "x  y");
    }

    #[test]
    fn test_table_zwsp_lets_columns_shrink() {
        let url = break_long_tokens("https://example.com/a/very/long/path");
        let d = TableData {
            aligns: vec![Alignment::None],
            header: TableRow {
                cells: vec![cell("u")],
                line: 1,
            },
            rows: vec![TableRow {
                cells: vec![Cell {
                    markup: url.clone(),
                    plain: url,
                }],
                line: 2,
            }],
        };
        let l = plan(&d, 100.0);
        assert!(!l.overflow, "broken URL has short minimum width");
    }

    #[test]
    fn test_table_emit_uses_table_header_and_alignment() {
        let mut d = data(&["A", "B", "C"], &[&["1", "2", "3"], &["4", "5"]]);
        d.aligns = vec![Alignment::Left, Alignment::Center, Alignment::Right];
        let l = plan(&d, W);
        let text: Vec<String> = emit(&d, &l).into_iter().map(|(t, _)| t).collect();
        let joined = text.join("\n");
        assert!(joined.contains("table.header([A], [B], [C]),"), "{joined}");
        assert!(joined.contains("align: (left, center, right,),"));
        assert!(joined.contains("  [1], [2], [3],"));
        assert!(
            joined.contains("  [4], [5], [],"),
            "short rows are padded: {joined}"
        );
        assert!(joined.contains("#set text(size: 8.5pt)"));
        // Exactly one table.header, never a plain first row.
        assert_eq!(joined.matches("table.header").count(), 1);
    }

    #[test]
    fn test_table_emit_line_map() {
        let d = data(&["A"], &[&["1"], &["2"]]);
        let l = plan(&d, W);
        let lines = emit(&d, &l);
        let header_line = lines
            .iter()
            .find(|(t, _)| t.contains("table.header"))
            .unwrap()
            .1;
        assert_eq!(header_line, 10);
        let rows: Vec<usize> = lines
            .iter()
            .filter(|(t, _)| t.starts_with("  ["))
            .map(|(_, l)| *l)
            .collect();
        assert_eq!(rows, [12, 13]);
    }

    #[test]
    fn test_table_fractions_sum_to_one_for_degenerate_input() {
        let d = data(&["", ""], &[&["", ""]]);
        let l = plan(&d, W);
        assert!((l.fractions.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(l.fractions.iter().all(|f| *f >= 0.0));
    }

    // ---- property tests (plan 8.3) ------------------------------------------------------

    use proptest::prelude::*;

    proptest! {
        /// Random column counts (1-10) and cell texts: fractions sum to 1.0 and are >= 0.
        #[test]
        fn fractions_sum_to_one_and_are_non_negative(
            columns in 1usize..=10,
            rows in prop::collection::vec(prop::collection::vec("[a-z .]{0,80}", 10), 0..6),
            width in 50.0f64..600.0,
        ) {
            let header = TableRow { cells: (0..columns).map(|i| cell(&format!("h{i}"))).collect(), line: 1 };
            let rows = rows
                .iter()
                .enumerate()
                .map(|(r, cells)| TableRow { cells: cells.iter().take(columns).map(|t| cell(t)).collect(), line: 2 + r })
                .collect();
            let data = TableData { aligns: vec![Alignment::None; columns], header, rows };
            let layout = plan(&data, width);
            prop_assert_eq!(layout.fractions.len(), columns);
            prop_assert!((layout.fractions.iter().sum::<f64>() - 1.0).abs() < 1e-9);
            prop_assert!(layout.fractions.iter().all(|f| *f >= 0.0 && f.is_finite()));
            prop_assert!(FONT_SIZES_PT.contains(&layout.font_size_pt));
        }

        /// Breaking tokens only ever inserts U+200B.
        #[test]
        fn break_long_tokens_only_inserts_zero_width_spaces(s in r"\PC{0,120}") {
            let broken = break_long_tokens(&s);
            prop_assert_eq!(broken.replace(ZWSP, ""), s.replace(ZWSP, ""));
        }
    }
}
