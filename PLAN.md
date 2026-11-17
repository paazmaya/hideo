# md2pdf: A Standalone Rust CLI for Building PDF Books from Markdown

## Complete Implementation Plan (Revised for Typst 0.15.1)

**Status:** Revised plan for review. Based on the original plan, with corrections for Typst 0.15.1 (released July 2026), additional detail on risky areas, and a more rigorous testing strategy.

---

## 1. Executive Summary

`md2pdf` is a single-binary Rust tool that converts a folder of Markdown pages and images into one PDF book per tier folder. It embeds the Typst compiler and PDF exporter in-process, so the only runtime dependency is the binary itself. The tool prioritizes reproducibility, clear diagnostics, and strict adherence to a narrow Markdown profile.

**Key decisions in this revision:**

| Area | Decision |
|------|----------|
| Typst version | Pin to **0.15.1** (released July 17, 2026)  |
| Font strategy | Use `typst-kit` with embedded fonts feature; Libertinus Serif and DejaVu Sans Mono are confirmed embedded  |
| World implementation | Hand-rolled `World` (not `typst-kit` helpers); the 0.15.0 changelog notes `typst-kit` was "completely reworked"  |
| Breaking changes | Typst 0.15.0 introduced breaking changes (baselines, paths, symbols); audit template and emitter accordingly  |
| pulldown-cmark risk | **New risk identified**: 0.13.4 has a known panic in `OffsetIter` with certain control characters. Mitigation: input sanitization + `catch_unwind` |
| Native Mermaid renderer | `mermaid-rs-renderer` (mmdr) exists but output quality may not match mermaid-cli. Gate at M7 remains critical |
| Release automation | `cargo-dist` 0.30.x with explicit target list |

---

## 2. Goals, Success Criteria, and Non-Goals

### 2.1 Goal

One self-contained binary that turns a source folder of Markdown pages and images into one PDF book per tier folder, with working internal links, a contents page, covers, hero images, diagrams and styled tables.

### 2.2 Success Criteria

1. `cargo install --path .` or a downloaded binary is all a contributor needs.
2. Every requirement in section 4 is met and has a test.
3. A full build of ~230 pages and ~260 MB of images takes ≤ 60 seconds on a 2024-era laptop; warm rebuild ≤ 10 seconds.
4. A Markdown mistake produces an error naming the page and line.
5. Two builds of the same source are byte-identical.

### 2.3 Non-Goals

- General Markdown support. Only the profile in section 5 is supported. Anything else is an error.
- A redesign of the visual look. Appendix B is the specification.
- Editing, serving, or previewing documents.

---

## 3. Architecture Decision: Embedded Typst Compiler

### 3.1 Decision

Parse Markdown with `pulldown-cmark`, emit Typst markup, compile in-process with the `typst` crate (version 0.15.1), export PDF with `typst-pdf` (version 0.15.1).

### 3.2 Rationale

- Typst is itself a Rust project with a stable library API pattern: implement `World`, call `typst::compile`, export with `typst_pdf::pdf`.
- Embedding avoids the external Typst CLI dependency.
- The `World` trait is small: main source, virtual files (images, SVGs), embedded fonts.
- Typst 0.15.1 fixes several regressions from 0.15.0, including alignment and layout bugs . The 0.15.x series also improved the `typst-kit` API for creating `World` implementations .

### 3.3 Consequences and Mitigations

| Consequence | Mitigation |
|-------------|------------|
| Typst API changes between releases | Pin exact version `0.15.1`. Confine all Typst use to `world.rs` and `compile.rs`. Upgrade only deliberately, with full test suite passing. |
| Typst 0.15 breaking changes | Audit template and emitter against the 0.15 migration guide: baseline handling, path separators (forward slashes only), symbol renames, `typst-kit` API changes . |
| `typst-kit` API reworked in 0.15 | The 0.15.0 changelog states `typst-kit` was "completely reworked to make it easier to create a Typst World implementation" . **Verify** the new API on docs.rs for 0.15.1. Hand-roll if the new API is awkward. |
| Font embedding must be reliable | `typst-kit` embedded fonts feature provides Libertinus Serif, DejaVu Sans Mono, and New Computer Modern . Confirm these exact families are available for 0.15.1. |
| PDF export timestamp | `typst_pdf::pdf` takes export options. Verify the `Timestamp`/`PdfOptions` API for 0.15.1 . Pass a fixed `Datetime` for reproducibility. |
| Non-Unicode paths unsupported | Typst 0.15 dropped support for non-Unicode input paths . Ensure all paths are valid UTF-8 before registering virtual files. |

### 3.4 Version Pinning Strategy

```toml
[dependencies]
typst = "=0.15.1"
typst-pdf = "=0.15.1"
typst-kit = { version = "=0.15.1", features = ["embedded-fonts"] }
typst-assets = "=0.15.1"
```

**Note on Typst 0.15.1:** Released July 17, 2026, it fixes math alignment regressions, `op` element vertical misalignment, and SVG pretty-printing bugs . These are minor relative to the 0.15.0 breaking changes but justify using 0.15.1 rather than 0.15.0.

---

## 4. Functional Requirements (Expanded)

*Requirements F1–F23 are largely as specified in the original plan. The following additions and clarifications apply.*

### 4.1 Clarifications and Additions

**F4 (Frontmatter) — Additional detail:**
- The hand-written parser must handle:
  - UTF-8 BOM at the start of the file (strip before parsing).
  - Windows CRLF (`\r\n`) and lone CR (`\r`) line endings; normalize to `\n` internally.
  - Quoted values with escaped quotes (`\"` inside double quotes, `\'` inside single quotes).
  - Values containing colons (`key: value: with colon`) — split on the *first* colon only.
- Error behavior: if `---` appears but no closing `---`, treat as a frontmatter error naming the page and line 1.

**F5 (Heading IDs) — Slug algorithm precision:**
- Unicode-aware: use `char::is_alphanumeric()` for letters/digits, plus explicit allowance for `_` and `-`.
- Spaces become hyphens; consecutive spaces do not collapse (two spaces → two hyphens).
- After slugification, if the result is empty (e.g., heading text was all punctuation), use `section` as a fallback ID.
- Explicit `{#id}` values are used verbatim (not slugified), but must match `^[a-zA-Z][a-zA-Z0-9_-]*$` to be a valid Typst label; otherwise error naming the page and line.

**F7 (Link rewriting) — Additional edge cases:**
- Links with URL-encoded characters (`%20`) in the path: decode before resolution.
- Links to directories: treat as unresolved (not Markdown pages).
- Links with only a fragment (`#section`): resolve within the current page.
- Case sensitivity: paths are resolved case-insensitively on Windows, case-sensitively on Linux/macOS (use `std::path` semantics, but warn if a case-insensitive match exists on a case-sensitive platform).

**F10 (Page head) — Hero image pull-into-margins:**
- The hero image spans the full page width (210 mm). In Typst 0.15, the `box` and `block` elements' purpose has been aligned with paged export . Use `#place(dx: -margin-x, dy: -margin-y)` with explicit dimensions, or a block with page margin overrides. Test this carefully in M3 against Typst 0.15.1.

**F15 (Diagrams) — Error message format:**
- Error must include: page path, diagram first line (truncated to 60 chars), and backend name (native/external). Example: `error: diagram failed to render\n  page: required/01_intro.md\n  line: 47\n  first line: graph TD; A-->B\n  backend: external (mermaid-renderer)`

**F19 (Reproducibility) — Timestamp details:**
- `SOURCE_DATE_EPOCH` is a Unix timestamp (seconds since epoch). Convert to `chrono::DateTime<Utc>` and pass to `typst_pdf` export options.
- If unset, use `2000-01-01T00:00:00Z` (a constant that is clearly not a real build time).
- **Important:** Typst 0.15.x may write `CreationDate` and `ModDate` in the PDF info dictionary. Verify that passing a fixed timestamp produces byte-identical output across two builds; if not, post-process the PDF to normalize those fields. The `typst-py` 0.15.0 release added a "reproducible pdf timestamp option," suggesting this is a supported concern .

**F23 (Check mode) — Expanded scope:**
- Check mode should also validate:
  - That every `hero` image file exists and is readable.
  - That every cover pattern resolves to at least one file (or warn if placeholder will be used).
  - That diagram sources are non-empty.
  - That heading IDs are unique within each page (F5).
  - That every frontmatter has `title` and `hero`.
- Output format: `--format json` produces a structured report with `errors`, `warnings`, and `stats` arrays.

---

## 5. Markdown Profile (Clarified)

The profile from the original plan is retained. Key clarifications:

| Construct | Clarification |
|-----------|---------------|
| Dollar signs | Always literal. Do **not** pass `ENABLE_MATH` to pulldown-cmark. |
| Footnotes, definition lists, strikethrough, etc. | Detect and error. pulldown-cmark will parse some of these by default; disable all extensions except `TABLES` and `STRIKETHROUGH` (for detection), then error if strikethrough events appear. |
| Inline HTML | pulldown-cmark emits `Event::Html` and `Event::InlineHtml`. Both are errors (block) or warnings (inline). |
| HTML comments | pulldown-cmark emits `Event::Html` for `<!-- ... -->`. Detect the comment pattern and drop silently. |
| Smart punctuation | Do **not** enable `ENABLE_SMART_PUNCTUATION`. Let Typst handle quotes and dashes. Manually replace `...` with `…` in text events. |

### 5.1 pulldown-cmark Version and Known Issue

**Pin to `pulldown-cmark = "=0.13.4"`.**

**Critical known issue:** Version 0.13.4 has a panic in `OffsetIter::next` when certain control characters (NUL, vertical tab, form feed) appear in link reference definitions. This is a real crash, not a theoretical risk.

**Mitigation:**
1. Before passing any page to pulldown-cmark, scan for and reject or strip NUL (`\0`), vertical tab (`\x0B`), and form feed (`\x0C`). Log a warning naming the page and line.
2. Wrap the parse in `std::panic::catch_unwind`. If a panic occurs, report a content error with the page name (the panic message may not include the line).
3. Monitor pulldown-cmark releases for 0.13.5 with the upstream fix.

---

## 6. Crates (Revised)

| Crate | Version | Use | Note |
|-------|---------|-----|------|
| `clap` | 4.x | CLI | derive feature |
| `serde` + `serde_derive` | 1.x | Config, reports | |
| `toml` | 0.8 | Config file | |
| `serde_json` | 1.x | JSON output, diagram theme | |
| `pulldown-cmark` | **=0.13.4** | Markdown parsing | See 5.1 for known issue |
| `image` | 0.25 | Hero images | `imageops::resize` with Lanczos3; `JpegEncoder` |
| `typst` | **=0.15.1** | Compiler | Pin exactly  |
| `typst-pdf` | **=0.15.1** | PDF export | Pin exactly  |
| `typst-kit` | **=0.15.1** | Fonts | `embedded-fonts` feature; API reworked in 0.15  |
| `typst-assets` | **=0.15.1** | Assets | |
| `walkdir` | 2.x | Page discovery | |
| `rayon` | 1.x | Parallel image/diagram work | |
| `blake3` | 1.x | Cache keys | |
| `anyhow` + `thiserror` | 1.x / 2.x | Errors | |
| `chrono` | 0.4 | Timestamp handling | For `SOURCE_DATE_EPOCH` |
| `tempfile` | 3.x | External diagram scratch | |
| `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Logging (optional) | |
| **dev:** `assert_cmd` | 2.x | CLI tests | |
| **dev:** `predicates` | 3.x | CLI assertions | |
| **dev:** `insta` | 1.x | Snapshot tests | |
| **dev:** `proptest` | 1.x | Property tests | |
| **dev:** `lopdf` | 0.44 | PDF inspection | `get_page_annotations` for link verification |

### 6.1 Considered but Deferred

| Crate | Reason |
|-------|--------|
| `jpeg-rusturbo` | ~3× faster JPEG encoding than `image`. Defer unless M8 performance testing shows image encoding is a bottleneck. |
| `comrak` | Alternative Markdown parser with AST. `pulldown-cmark`'s event API is sufficient for the profile; `comrak`'s AST would only help if table parsing becomes unwieldy. Gate: M5 table implementation. |
| `mermaid-rs-renderer` | Pure-Rust Mermaid renderer, very fast (ms per diagram), but output quality may not match mermaid-cli. Evaluate at M7 gate. |
| `cargo-dist` | For release automation. Add at M9. |

---

## 7. Design (Expanded)

### 7.1 Crate Layout

```
md2pdf/
  Cargo.toml
  md2pdf.toml                  # example config
  assets/
    template.typ               # page styles, helpers
    mermaid-theme.json         # default diagram theme
  src/
    main.rs                    # entry point, exit codes
    cli.rs                     # clap definitions
    config.rs                  # serde structs, defaults, validation
    error.rs                   # error/warning types with page+line
    discover.rs                # find and order pages (F2, F3)
    frontmatter.rs             # F4 parser
    page.rs                    # Page struct, heading extraction
    linkmap.rs                 # F5–F8
    markdown/
      mod.rs                   # parse, event walk
      emit.rs                  # events → Typst markup
      escape.rs                # Typst escaping
      tables.rs                # F16
      lead.rs                  # F17
    images.rs                  # heroes, covers, cache
    diagrams/
      mod.rs                   # DiagramRenderer trait, cache
      native.rs                # pure-Rust backend
      external.rs              # runs external command
    template.rs                # fills tokens, builds cover/contents/page heads
    world.rs                   # typst::World implementation
    compile.rs                 # compile, export, diagnostic mapping
    report.rs                  # unresolved links, warnings, timings
  tests/
    fixtures/                  # small books covering every construct
    integration/               # assert_cmd + lopdf tests
```

### 7.2 Data Flow for One Tier (Revised)

1. **Discover and order** pages (F2, F3). Read each file once. Normalize CRLF.
2. **Sanitize** each file's bytes for control characters (NUL, VT, FF) before parsing.
3. **Parse frontmatter** (F4). Validate `title` and `hero` presence.
4. **Parse body** with `pulldown-cmark`, collecting events. Detect unsupported constructs and emit errors.
5. **Compute heading IDs** for every page (F5). Build label map for the tier.
6. **Rewrite links** (F7). Record unresolved entries.
7. **Emit Typst markup** for each page body.
8. **Prepare images in parallel:** heroes, cover, contents hero (F11, F12).
9. **Render diagrams in parallel**, deduplicated by hash (F15).
10. **Assemble Typst source:** template + cover + contents + (page head + body) per page. Maintain source map.
11. **Compile and export** with fixed timestamp. Write `{tier}.pdf`.
12. **Map diagnostics** back to page files.
13. **Print warnings** and unresolved-link report.

### 7.3 Command Line (Expanded)

```
md2pdf build [TIER...]
    -i, --input <DIR>           Source folder (default: .)
    -o, --output <DIR>          Output folder (default: pdf-output)
    -c, --config <FILE>         Config file (default: md2pdf.toml in source folder)
        --diagrams <BACKEND>    native | external | off (default: external)
        --keep-typst <DIR>      Write generated Typst source per tier
        --deny-unresolved       Exit 1 if any internal link is unresolved
        --jobs <N>              Worker threads (default: logical cores)
    -q, --quiet                 Suppress warnings
    -v, --verbose               Verbose logging

md2pdf check [TIER...]
        --format <text|json>    Report format (default: text)

md2pdf list [TIER...]
        --format <text|json>    Print pages in PDF order
```

**Exit codes:**
- `0`: Success.
- `1`: Build or content error (missing title, unsupported construct, compile error).
- `2`: Usage or configuration error (bad CLI args, invalid TOML).

### 7.4 Configuration (Expanded)

The TOML structure from Appendix A is retained. Additions:

```toml
[build]
# New: cache directory (default: .md2pdf-cache in output folder)
cache_dir = ".md2pdf-cache"

[diagnostics]
# New: treat warnings as errors
warnings_as_errors = false
```

**Validation rules (config.rs):**
- `tiers` must be non-empty.
- `page.width_mm` must be > `2 * page.margin_x_mm`.
- `images.jpeg_quality` must be 1–100.
- `diagrams.backend` must be one of `native`, `external`, `off`.
- If `backend = "external"`, `diagrams.command` must be non-empty.

### 7.5 Markdown to Typst Emission (Expanded)

#### Escaping (escape.rs)

This is the highest-risk module. The rule set must be exhaustive:

**Text mode escaping:**
| Character | Escape as | Reason |
|-----------|-----------|--------|
| `\` | `\\` | Typst escape |
| `#` | `\#` | Markup/script |
| `$` | `\$` | Math |
| `@` | `\@` | Reference |
| `*` | `\*` | Strong/emphasis |
| `_` | `\_` | Emphasis |
| `` ` `` | `` \` `` | Raw |
| `<` | `\<` | Label |
| `>` | `\>` | Label |
| `[` | `\[` | Content block |
| `]` | `\]` | Content block |
| `~` | `\~` | Non-breaking space |
| `//` | `\/\/` | Line comment |
| `/*` | `\/*` | Block comment |
| `=` at line start | `\=` | Heading |
| `-` at line start | `\-` | List |
| `+` at line start | `\+` | List |
| `/` at line start | `\/` | Term list |
| `N.` at line start | `\N.` | Enum |

**Raw block escaping:** Use raw strings with delimiter longer than any backtick run in the content. Example: if content contains ```` ``` ````, use ```` ```` ```` as delimiter.

**Property test (proptest):**
```rust
proptest! {
    #[test]
    fn emitted_markup_always_compiles(s in "\\PC*") {
        let typst_source = emit_text(&s);
        let world = TestWorld::new(typst_source);
        let result = typst::compile(&world);
        prop_assert!(result.is_ok(), "Failed to compile: {:?}", result.err());
    }
}
```

#### Tables (tables.rs)

The algorithm from the original plan is retained with these clarifications:

- **Text width:** `page_width_mm - 2 * margin_x_mm` converted to points (1 mm = 2.834645669 pt).
- **Column width fractions:** After computing preferred and minimum widths, normalize to fractions summing to 1.0. Typst's `columns: (1fr, 2fr, ...)` syntax requires fractions that sum to any positive number; normalization is for readability.
- **Font sizes:** Try 8.5, 7.5, 7.0 pt in order. The first size where `sum(min_widths) <= text_width` wins. If none fits, use 7.0 pt and allow overflow (log a warning).
- **Long strings:** Insert zero-width space (`\u{200B}`) after `.`, `/`, `_`, `-`, `?`, `=`, `&` in strings of length ≥ 16. **Important:** This must happen *before* escaping, and the zero-width space must not be escaped.

#### Lead-in paragraphs (lead.rs)

Detection: A paragraph whose first child is `Event::Start(Strong)` followed by text ending in `.` or `:`, and the strong content is ≤ 40 characters.

Emission: `#lead[Strong text]` followed by the rest of the paragraph.

**Edge case:** If the strong text is exactly 40 characters and ends with `.`, it qualifies. If the strong content spans multiple text events (e.g., `**A** **B.**`), do not treat as lead-in (the strong is not contiguous).

### 7.6 Images (images.rs)

**Hero image processing pipeline:**

1. Resolve path: next to page first, then relative to source folder.
2. Read file bytes.
3. Decode with `image::load_from_memory`.
4. Convert to RGB8 (drop alpha, composite onto white if needed).
5. Compute target width: `page_width_mm * dpi / 25.4` pixels.
6. If image width > target width, resize with Lanczos3.
7. Encode as JPEG at configured quality.
8. Store in `HashMap<PathBuf, Vec<u8>>` as virtual file.

**Cache key:** `blake3(file_bytes + settings_json)`. Cache stored in `output_dir/.md2pdf-cache/images/{hash}.jpg`.

**Placeholder (missing hero):** Generate a 16:9 light grey rectangle as JPEG in memory. No disk cache for placeholders.

### 7.7 Diagrams (diagrams/mod.rs)

```rust
pub trait DiagramRenderer: Send + Sync {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError>;
    fn name(&self) -> &'static str;
}
```

**External backend:** Write source to temp file, invoke command with theme path, read SVG from stdout or output file. Timeout: 30 seconds per diagram. Capture stderr on failure.

**Native backend:** Wrap `mermaid-rs-renderer` (mmdr). **Important:** mmdr's own documentation states "Visual output quality is improving rapidly but may not yet match mermaid-cli in all cases." The M7 gate is essential.

**M7 Gate protocol:**
1. Render all ~120 diagrams in the target content with:
   - External backend (mermaid-cli or configured command).
   - Each native candidate.
2. Create a contact sheet (grid of rendered images with labels).
3. Manual review: any diagram that fails to render, or whose native output is visibly worse than external, disqualifies that native backend.
4. If no native backend passes, ship with external backend as default and document "standalone except for diagrams."

### 7.8 Typst World (world.rs)

**Important for Typst 0.15:** The `typst-kit` crate was "completely reworked" in 0.15.0 . The new API should make it easier to create a `World`. **Verify** the exact API on docs.rs for 0.15.1 before implementing. If the new API is straightforward, use it; otherwise, hand-roll.

Minimal implementation sketch:

```rust
pub struct Md2PdfWorld {
    main: Source,
    files: HashMap<FileId, Bytes>,
    fonts: FontBook,
    font_slots: Vec<FontSlot>,
}

impl World for Md2PdfWorld {
    fn library(&self) -> &LazyHash<Library> { &LIBRARY }
    fn book(&self) -> &LazyHash<FontBook> { &self.fonts }
    fn main(&self) -> FileId { self.main.id() }
    fn source(&self, id: FileId) -> FileResult<Source> { ... }
    fn file(&self, id: FileId) -> FileResult<Bytes> { ... }
    fn font(&self, index: usize) -> Option<Font> { ... }
    fn today(&self, offset: Option<i64>) -> Option<Datetime> { Some(FIXED_DATETIME) }
}
```

**Font loading:** Use `typst_kit::fonts::FontSearcher` with `include_system_fonts(false)` and `include_embedded_fonts(true)`. This gives Libertinus Serif, DejaVu Sans Mono, and New Computer Modern . Verify that the 0.15.1 `typst-assets` includes these.

**Virtual files:** Register images and SVGs with `FileId::new(None, VirtualPath::new("images/hero1.jpg"))`. The main source is `FileId::new(None, VirtualPath::new("main.typ"))`.

**Path separator:** Typst 0.15 requires forward slashes in paths, even on Windows . Convert all paths before registering.

### 7.9 Compile and Export (compile.rs)

```rust
let world = Md2PdfWorld::new(...);
let result = typst::compile(&world);
match result.output {
    Ok(document) => {
        // Verify PdfOptions API for 0.15.1 on docs.rs
        let options = PdfOptions {
            timestamp: Some(FIXED_TIMESTAMP),
            ..Default::default()
        };
        let pdf_bytes = typst_pdf::pdf(&document, &options);
        std::fs::write(output_path, pdf_bytes)?;
    }
    Err(errors) => {
        // Map each error's span to a page and line via source map
    }
}
```

**Source map:** During assembly of the Typst source, record for each line range which page file and Markdown line range it came from. When Typst reports a diagnostic with a span, convert `span` → byte offset in `main.typ` → line number → page file + Markdown line.

---

## 8. Testing Strategy (Expanded)

### 8.1 Test Levels

| Level | What | Tool | Coverage Target |
|-------|------|------|-----------------|
| 1 | Unit tests per requirement | `cargo test` | 100% of F1–F23 logic |
| 2 | Emitter snapshots | `insta` | Every construct in section 5 |
| 3 | Escaping property | `proptest` | 10,000 random strings compile |
| 4 | Table algorithm | Unit tests | Hand-computed expected widths |
| 5 | End-to-end PDF inspection | `assert_cmd` + `lopdf` | Page count, links, text extraction |
| 6 | Reproducibility | Integration test | Byte-identical outputs |
| 7 | Visual review | Manual | 25-page sample vs. Appendix B |
| 8 | Cross-tool validation | Script | Pandoc+Typst CLI comparison |

### 8.2 Detailed Test Cases by Requirement

#### F1–F4: Discovery and Frontmatter

```
test_tiers_from_config
test_empty_tier_skipped_with_warning
test_ignore_prefixes_at_depth
test_page_order_case_insensitive_natural
test_frontmatter_basic
test_frontmatter_quotes_stripped
test_frontmatter_duplicate_key_first_wins
test_frontmatter_indented_lines_ignored
test_frontmatter_crlf
test_frontmatter_missing_title_errors
test_frontmatter_missing_hero_errors
test_frontmatter_utf8_bom_stripped
test_frontmatter_colon_in_value
test_frontmatter_escaped_quotes
```

#### F5–F8: Links

```
test_heading_id_slug_unicode (ä, ö, å)
test_heading_id_explicit
test_heading_id_duplicate_suffix
test_heading_id_empty_fallback_section
test_heading_inside_code_ignored
test_page_labels_unique_across_pages
test_link_external_unchanged
test_link_non_md_file_unchanged
test_link_same_page_anchor
test_link_same_page_no_anchor_to_first_heading
test_link_cross_page
test_link_cross_page_anchor
test_link_outside_pdf_marked
test_link_missing_anchor_marked
test_link_directory_unresolved
test_report_grouped_by_pdf_sorted
test_deny_unresolved_exits_1
```

#### F9–F18: Content

```
test_markdown_profile_all_constructs
test_unsupported_footnote_errors
test_unsupported_strikethrough_errors
test_unsupported_inline_html_warns
test_html_comment_dropped_silently
test_dollar_literal
test_hero_scaled_not_upscaled
test_hero_missing_placeholder_warning
test_cover_full_bleed
test_contents_two_columns
test_contents_groups_by_folder
test_contents_omits_pages_without_headings
test_page_number_on_all_but_cover
test_diagram_external_renders
test_diagram_failure_names_page_and_line
test_table_narrow_one_size
test_table_wide_reduced_font
test_table_long_word_breaks
test_table_header_repeats (lopdf: check table header text on multiple pages)
test_lead_in_label
test_external_link_marker
```

#### F19–F23: Output and Behavior

```
test_reproducible_two_builds_identical
test_timestamp_from_source_date_epoch
test_error_names_page_and_line
test_warnings_collected_and_counted
test_check_mode_no_pdf_written
test_check_mode_json_format
test_exit_codes
test_config_unknown_key_errors
test_config_defaults
```

### 8.3 Property Tests

| Property | Input | Assertion |
|----------|-------|-----------|
| Escaping | Random Unicode string | Emitted Typst compiles without error |
| Table widths | Random column counts (1–10), random text lengths | Fractions sum to 1.0, each ≥ 0 |
| Slug uniqueness | Random heading lists | All IDs unique |
| Link rewriting | Random path + anchor combinations | Output is either unchanged or valid Typst link |

### 8.4 End-to-End Fixtures

Create fixture books:

1. **`minimal/`** — 2 pages, 1 tier, no images, no diagrams. Tests basic flow.
2. **`full/`** — 20 pages, 3 tiers, all constructs, 5 diagrams, 10 images. Tests integration.
3. **`edge/`** — Pages with Unicode headings, long table strings, missing heroes, unresolved links. Tests error/warning paths.
4. **`repro/`** — Same as `minimal/` but built twice. Tests byte-identical output.

### 8.5 Visual Review Protocol (M6, M8)

1. Build `full/` fixture.
2. Render pages 1 (cover), 2 (contents), 3 (first content page), and a table-heavy page to PNG (using `pdftoppm` or `lopdf` + image).
3. Compare side-by-side with Appendix B specifications.
4. Check: font sizes, colors, margins, spacing, table lines, quote bar, code block radius.
5. Record any deviations as issues. Fix before M8.

### 8.6 CI Pipeline

```yaml
# .github/workflows/ci.yml
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
      - run: cargo build --release
      - run: cargo run --release -- build --input tests/fixtures/full --output /tmp/out
```

**Note:** Windows CI is essential because of path handling, CRLF, and the Typst 0.15 forward-slash requirement.

---

## 9. Milestones (Revised)

| M | Milestone | Requirements | Done When | Days |
|---|-----------|--------------|-----------|------|
| 1 | Skeleton + Config | F1–F4, F21, F22 | CLI parses, config loads, discovery + frontmatter work, `list` and `check` on fixtures | 2 |
| 2 | Link Model | F5–F8 | All link tests pass, report format correct | 2 |
| 3 | Images | F10–F12 | Heroes scaled/cached/placeholdered, cover generated | 2 |
| 4 | Emitter Core | F9, F17, F18 | Headings, paragraphs, emphasis, links, lists, quotes, code, rules, escaping; snapshot + property tests pass | 5 |
| 5 | Tables | F16 | Table fixtures pass, font reduction works, long words break | 4 |
| 6 | Typst Integration | F10, F12–F14, F19, F20 | World, template, cover, contents with page numbers, PDF export, reproducible, error mapping | 5 |
| 7 | Diagrams | F15 | Trait, external backend, native evaluation, **M7 gate decision** | 4 |
| 8 | Hardening | All | Full content builds, performance goal met, visual review pass | 4 |
| 9 | Packaging | F21, F23 | `--help` polish, README, CI, `cargo-dist` release config | 2 |
| | **Total** | | | **30** |

**Order notes:**
- M4 and M5 must complete before M6, so emitter bugs are caught before Typst compilation.
- M7 can run in parallel with M5–M6 (diagrams are independent of table/image work).
- M3 (images) depends on M6 (Typst) for testing hero placement, but the image processing logic can be written in parallel.
- **M6 is riskier with Typst 0.15:** Budget extra time for learning the reworked `typst-kit` API and auditing the template for 0.15 breaking changes .

---

## 10. Risks (Expanded)

| Risk | Likelihood | Effect | Mitigation |
|------|------------|--------|------------|
| Typst API changes | Medium | Rework in `world.rs`/`compile.rs` | Pin 0.15.1; confine Typst; upgrade deliberately |
| Typst 0.15 breaking changes break template | **High** | Visual regressions, compile errors | Audit template against migration guide ; visual review at M6 |
| `typst-kit` reworked API is awkward | Medium | Hand-roll `World` | Verify new API first; fall back to hand-rolled `World` |
| pulldown-cmark 0.13.4 panic | **High** (known bug) | Crash on certain inputs | Sanitize control chars; `catch_unwind`; monitor 0.13.5 |
| No native Mermaid renderer passes gate | Medium | Not fully standalone | External backend stays; document limitation |
| Hand-written emitter edge case | **High** | Broken pages | Fixtures, snapshots, property test, strict errors |
| Typst markup injection | Medium | Build errors or wrong output | Single escaping module; property test |
| Table widths wrong | Medium | Page breaks shift | Literal algorithm, hand-computed fixtures, visual review |
| Font embedding fails in release build | Low | Wrong fonts on user machines | `typst-kit` embedded-fonts feature; CI build test |
| Windows path/CRLF issues | Medium | Failures on Windows | `std::path` throughout; normalize CRLF; Windows CI; forward slashes for Typst paths |
| Large inputs memory/time | Low | Out-of-memory, slow builds | Image cache, in-memory images, measure at M8 |
| Scope creep | Medium | Schedule slip | Unsupported-construct error keeps scope fixed |

---

## 11. Decisions Needed Before Starting

1. **Name and location.** A tooling folder inside the repository, or its own repository?
2. **Diagrams.** Accept "standalone except diagrams" if no native renderer passes the gate, or hold the release until one does?
3. **Visual tolerance.** Is it enough that Appendix B is followed and the checks in level 5 pass, or must the pages match a given set of reference PDFs line by line?
4. **Typst version.** Confirm pin to **0.15.1** (this revision's recommendation).
5. **Cover text.** The book title, author and the line template are configuration (Appendix A); confirm the wording.

---

## 12. Sources

- [Typst 0.15.1 changelog](https://typst.app/docs/changelog/0.15.1/) 
- [Typst 0.15.0 changelog and migration guide](https://typst.app/docs/changelog/0.15.0/) 
- [Typst 0.14.0 changelog](https://typst.app/docs/changelog/0.14.0/) 
- [typst-kit on docs.rs (fonts component)](https://docs.rs/typst-kit/0.12.0/typst_kit/index.html) 
- [typst-kit source (fonts components)](https://docs.rs/typst-kit/0.12.0-rc1/src/typst_kit/lib.rs.html) 
- [typst-pdf on docs.rs (PdfOptions, Timestamp)](https://docs.rs/typst-pdf/0.14.0/aarch64-unknown-linux-gnu/typst_pdf/) 
- [typst-py 0.15.0 release (reproducible PDF timestamp option)](https://github.com/messense/typst-py/releases/tag/v0.15.0) 
- [crates.io keyword search: typst (confirms 0.15.1 versions)](https://crates.io/keywords/typst?page=1&sort=downloads) 
- [Typst CLI reference (creation timestamp, pdf-standard)](https://github.com/lucifer1004/claude-skill-typst/blob/dcfe6144f6e5f6576fb55cf1a6b9970851a1aa78/skills/typst/cli.md) 
- [Typst as library: a World, compile and PDF export](https://github.com/tfachmann/typst-as-library)
- [Typst forum: using the compiler as a library](https://forum.typst.app/t/how-do-i-use-the-typst-compiler-in-rust-as-a-library/6859)
- [ariel-rs](https://docs.rs/ariel-rs), [mermaid-rs-renderer](https://docs.rs/mermaid-rs-renderer), [merman](https://libraries.io/cargo/merman-wasm): pure-Rust Mermaid renderers
- [serde_yaml deprecation and alternatives](https://users.rust-lang.org/t/serde-yaml-deprecation-alternatives/108868), [serde_yml notice](https://docs.rs/crate/serde_yml/0.0.13), [serde-saphyr](https://docs.rs/crate/serde-saphyr/0.0.25)

---

## Appendix A: Configuration Example

```toml
# md2pdf.toml
tiers = ["required", "additional", "bonus"]   # folder names; also the PDF names
ignore_prefixes = ["_", "."]

[page]
width_mm = 210
margin_x_mm = 20
margin_y_mm = 18
hero_height_mm = 118.125        # placeholder height, 16:9

[images]
dpi = 200                       # heroes are scaled to the page width at this resolution
jpeg_quality = 70
contents_hero = "images/contents.png"
cover_pattern = "images/cover_front_{tier}.png"   # grey-violet placeholder when missing

[cover]
title = "Book title"
author = "Author name"
tier_line = "{Tier} course material"
panel_width_mm = 170
panel_offset_mm = 20

[contents]
columns = 2

[diagrams]
backend = "external"            # "native", "external" or "off"
command = "mermaid-renderer"    # used by the external backend
theme = "assets/mermaid-theme.json"

[build]
cache_dir = ".md2pdf-cache"

[diagnostics]
warnings_as_errors = false
```

---

## Appendix B: Visual Specification

All sizes and colours below are requirements and live in the template (`assets/template.typ`), with the page geometry filled in from the configuration.

| Element | Specification |
|---------|---------------|
| Page | A4 portrait, margins from the configuration; footer: page number centred, 8 pt, `#6E6682` |
| Body text | Libertinus Serif 10.5 pt, `#2A2438`, not justified, leading 0.75 em, paragraph spacing 0.65 em, language from frontmatter |
| Heading 1 | 19 pt bold, `#24134D`, 1.6 em above and 1.5 em below, kept with the next block |
| Heading 2 | 15 pt bold, `#3D2A7A`, 2.4 em above and 1.4 em below, kept with the next block |
| Heading 3 | 12 pt bold, `#52389A`, 1.8 em above and 1.1 em below, kept with the next block |
| Headings | not numbered |
| Lists | 1.3 em of space after a list |
| Code block | full width, fill `#F3F0F8`, 9 pt inset, 4 pt radius, DejaVu Sans Mono 8 pt |
| Links | `#4A2F8F`; links that leave the PDF are followed by a small arrow icon (an inline SVG, 0.6 em high, stroke `#4A2F8F`) |
| Block quote | full width, fill `#F0EBF8`, inset 8 pt vertical and 10 pt horizontal, a 2.5 pt left bar `#5B3FA0`, 3 pt radius on the right, 1 em above and below, text 10 pt, paragraph spacing 0.6 em |
| Lead-in label | bold, `#5B3FA0` |
| Thematic break | centred line, 30 % of the width, 0.6 pt, `#D3CCE4`, 1.4 em above and below |
| Tables | cell inset 4 pt by 4 pt; only horizontal lines, 0.4 pt `#D3CCE4`; header row fill `#E9E3F6` and bold; every second body row fill `#F5F3F9`; may break across pages; rows are not split when they fit; hyphenation on; 1.8 em above and below; header repeats on each page |
| Page head | hero full width pulled into the margins, 5 mm space; subtitle 12 pt `#6E6682` with 8 pt below; author line 9 pt `#6E6682` followed by 5 mm; placeholder fill `#E7E3EF` |
| Contents | title "Contents" 19 pt bold `#24134D`; two columns with 6 mm gutter; text 7.5 pt, leading 0.4 em; group label 8.5 pt bold `#3D2A7A`; entry: title, dotted leader, page number, text `#2A2438`, 0.55 em above |
| Cover | full-bleed, panel `#2E1A5C`, white text; title 24 pt bold, tier line 16 pt, author 12 pt; panel inset 16 pt, radius 4 pt; placeholder `rgb(231, 227, 239)` |

---

## Appendix C: Default Diagram Theme

Passed to the diagram renderer as JSON (the renderer must support the settings it is given; settings it does not know are ignored):

```json
{
  "theme": "base",
  "look": "neo",
  "themeVariables": {
    "fontSize": "22.4px",
    "radius": 8,
    "primaryColor": "#E9E3F6",
    "primaryTextColor": "#2E1A5C",
    "primaryBorderColor": "#5B3FA0",
    "secondaryColor": "#F3F0F8",
    "secondaryTextColor": "#2E1A5C",
    "secondaryBorderColor": "#8B74C8",
    "tertiaryColor": "#FFF3CC",
    "tertiaryTextColor": "#1A1A1A",
    "tertiaryBorderColor": "#B37A00",
    "lineColor": "#4A2F8F",
    "textColor": "#2E1A5C",
    "cScale0": "#D9CFF0",
    "cScale1": "#C9BDE8",
    "cScale2": "#E3DCF4",
    "cScaleLabel0": "#2E1A5C"
  },
  "flowchart": {
    "curve": "rounded",
    "padding": 24,
    "nodeSpacing": 40,
    "rankSpacing": 50,
    "wrappingWidth": 420
  },
  "mindmap": { "padding": 24, "maxNodeWidth": 260 },
  "block": { "style": "rounded" }
}
```

The three `cScale` colours repeat in turn for scales 3 to 11, and `cScaleLabel0` to `cScaleLabel11` are all `#2E1A5C`.