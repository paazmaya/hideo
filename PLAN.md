# hideo: A Standalone Rust CLI for Building PDF Books from Markdown

## Implementation Task List

> Evaluated 2026-11-18 against commit `28ed39c` ("Implementation has begun"). At that commit
> `cargo build` failed with 28 errors, `main.rs` was `todo!()` for all three commands, the Typst
> template was not valid Typst and several fixtures were malformed. Most modules were rewritten
> rather than repaired. `[x]` = implemented **and** verified by `cargo test` (166 unit tests,
> 58 integration tests, all green) or by running the binary; nothing is marked done because the
> code merely exists. `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are
> clean with and without `--features native-diagrams`.
>
> **Last updated 2026-11-18.** Nothing is committed yet.

**Foundation (Appendix E)**

- [x] `Cargo.toml` with pinned Typst crates, features and profiles; `Cargo.lock` committed (unused `anyhow`, `tracing*` and `image-compare` removed)
- [x] `rust-toolchain.toml`, `.gitignore` replaced, example `hideo.toml`
- [x] `.semaphore/semaphore.yml`, `.semaphore/release.yml`, `appveyor.yml` written; their commands match the real CLI flags
- [x] Config structs with `deny_unknown_fields` (`config.rs`), CLI definitions (`cli.rs`), error and warning codes (`error.rs`)
- [x] `oxlint.json` deleted; Renovate `packageRules` for `typst*` and `pulldown-cmark`; README rewritten (usage, profile, accessibility); CHANGELOG entry
- [ ] **CI actually green on Semaphore and AppVeyor** (needs a push; the first-run `(verify)` items in 8.6 are still open)
- [ ] README badge and docs links to `semaphore.io` (verify they redirect)

**M1 Skeleton + Config (F1–F4, F21, F22)**

- [x] Crate compiles with no errors; `cargo clippy` clean
- [x] Config loading from file, paths relative to the config file, validation, exit code 2 (`test_config_*`)
- [x] Discovery: ignore prefixes at any depth, natural case-insensitive order, non-UTF-8 rejection (F2, F3). The non-UTF-8 test only runs on Unix.
- [x] Frontmatter parser incl. BOM, CRLF, quotes, unclosed block (F4) with the unit tests in 8.2
- [x] `list` and `check` commands work on fixtures; exit codes 0/1/2
- [x] Fixtures rewritten as valid single-frontmatter pages (`minimal`, `full`, `edge`, `repro`, `windows-paths`, plus `diagrams`)
- [x] Day-1 vertical slice: `HideoWorld` compiles a hard-coded string to a PDF that `lopdf` reads

**M2 Link Model (F5–F8)**

- [x] Heading IDs: slug, explicit `{#id}`, duplicates, validation (E003)
- [x] Page-qualified labels `<page_key--id>`; lexical link resolution, case-mismatch warning, outside-tier, report grouping, `--deny-unresolved`

**M3 Images (F10–F12)**

- [x] Hero pipeline: resolve, EXIF, alpha, scale (never upscale), JPEG, blake3 cache, decode limits, placeholder (W003)
- [x] Cover and contents-hero images; resize benchmark recorded (notes below)

**M4 Emitter Core (F9, F17, F18, F5 labels)**

- [x] Escaping (`escape.rs`) incl. line-start rules; code goes into string literals instead of raw delimiters (note N3)
- [x] Event-walk emitter: paragraphs, emphasis, strong, links, lists (nested, ordered), quotes, code, rules, hard/soft breaks, body images
- [x] Unsupported-construct errors/warnings, HTML comment dropping, `catch_unwind`
- [x] Lead-in paragraphs; external-link arrow marker with alt text
- [x] `LineMap` (Typst line to Markdown line)
- [x] `insta` snapshots and `proptest` properties (escaping, table widths, slugs, line index, link destinations)

**M5 Tables (F16, F24)**

- [x] Width/fraction algorithm, font reduction, long-string ZWSP, alignment
- [x] Header row emitted as `table.header(...)`; W006
- [x] Struct-tree test with `lopdf`: header cells are `TH` with column scope, body cells `TD`, negative control
- [ ] Non-splitting rows: `set table.cell(breakable: false)` is in the template, but no fixture proves that a row that fits is never split across pages

**M6 Typst Integration (F10, F12–F14, F19, F20, F24)**

- [x] Valid `assets/template.typ`; cover, contents with page numbers and leaders as artifacts, footer
- [x] Per-page virtual files, `World` on `typst-kit`, fixed timestamp, `SOURCE_DATE_EPOCH` (exit 2 if invalid)
- [x] PDF/UA-1 validation on by default; E006 mapping; atomic PDF write
- [x] Diagnostic mapping to page and Markdown line (proved end to end by the missing-glyph E006 at the right file line)
- [x] `build` command end to end; `--keep-typst`; `--jobs`; reproducibility test across processes (`--jobs 1` vs `8`)

**M7 Diagrams (F15)**

- [x] `DiagramRenderer` trait, post-conditions (`<svg`, no `foreignObject`, font warning), disk cache
- [x] External backend with timeout and stderr tail; error format from F15
- [x] Mermaid fence handling in the emitter with alt text (W008)
- [x] Native backend (`native-diagrams`) and M7 gate evaluation against real `mmdc` 12.2.0 on nine diagram types (note N5)
- [x] External backend verified with real `mmdc` (`test_real_mmdc_output_passes_the_post_conditions`, skipped when `mmdc` is absent); the theme needed top-level `htmlLabels` and `fontFamily`
- [x] 120 diagrams through real `mmdc` 12.2.0 (generated: 9 types, 30 pages): 0 errors, cold 25 s on 32 cores, warm 1 s, two cold builds byte-identical (note N11). The author's actual book diagrams are still untested

**M8 Hardening**

- [x] Full-fixture build, PDF/UA-1 passes, `typst-render` visual snapshots in `tests/visual/` (8 pages, reviewed against Appendix B)
- [x] Performance benchmark on a generated corpus: goal met (note N6)
- [ ] veraPDF baseline (`--flavour ua1`) on the `full/` PDFs (manual; needs Java)
- [ ] One screen-reader pass (NVDA) over the cover, contents and a table-heavy page (manual)

**M9 Packaging**

- [x] `--help` polish, README install/usage, CHANGELOG
- [ ] Release pipeline (8.7) exercised with a real tag; archives, `SHA256SUMS`, draft release

### Implementation notes (2026-11-18)

Findings and deviations from the text below, in the order they matter:

- **N1. Typst quirks found by the property tests**, now handled in `escape.rs` and recorded in
  `proptest-regressions/`: a `-` before a digit becomes a minus sign (U+2212; `COVID-19` would not
  keep its hyphen), so it is escaped; after an embedded expression such as `#strong[x]`, a following
  `.name` is parsed as a field access, `(` as a call and `;` is swallowed as a terminator, so those
  are escaped when they come first (`Position::AfterExpr`); a digit followed by a quote becomes a
  prime. Inline emphasis uses `#emph[…]`/`#strong[…]`, not `_`/`*`, because Typst ignores
  intra-word delimiters.
- **N2. Page titles.** Decision D12 (new): every page gets a heading. If the Markdown body has no
  heading, the frontmatter `title` is emitted as an H1. The contents list one entry per page, so
  `test_contents_omits_pages_without_headings` does not exist.
- **N3. Code is emitted as `#raw("…", block: true, lang: "…")`**, a string literal, not as backtick
  delimiters (design B of 7.5, used where it costs nothing). Nothing in code can be read as markup
  and no delimiter can be forged. Prose still uses design A.
- **N4. Diagram sizing** (decision D13, new). `width: 100%` blew small diagrams up until they were
  clipped by the page height. Body images are scaled to the text width; diagrams are never scaled
  _above_ their natural size; both are capped at 85 % of the text height (`fitted-image` in
  `template.typ`). The theme's 22.4 px font assumes wide `mmdc` output that is scaled down.
- **N5. M7 gate, native backend (`mermaid-rs-renderer` 0.3.1), reviewed from rendered PDF pages at
  60 dpi.** It needs the theme's `"22.4px"` rewritten as a number (`normalise_theme`). Verdict per
  type: flowchart, flowchart-lr, sequence, class, state **pass** (theme colours and Libertinus
  Serif applied, no `foreignObject`); gantt is legible but crude; **er** has labels overlapping the
  relation lines, **pie** has near-white slices, **mindmap** uses off-theme bright colours: these three
  **fail**. **Update, same day, with `mmdc` 12.2.0 installed:** the side-by-side comparison on nine
  types (flowchart, flowchart-lr, sequence, class, state, er, pie, mindmap, gantt) showed that
  `mmdc` renders all nine on-theme with Libertinus Serif and no `foreignObject`. Native flowchart and
  state diagrams are equivalent (native has one crossing edge in the sample); native **sequence**
  (grey, off-theme participant boxes) and **class** (members run together on one line) lose the
  comparison. Outcome: `backend = "native"` renders only `graph|flowchart|stateDiagram` natively
  (`NATIVE_KINDS`) and hands every other type to the external `command` (`RoutingRenderer`);
  `diagrams.native_only = true` renders everything natively. `merman` was not evaluated.
  Two things only the real `mmdc` showed: mermaid 11+ ignores `flowchart.htmlLabels` and needs a
  top-level `htmlLabels: false`, else every flowchart carries `<foreignObject>` and hideo (correctly)
  fails with E004; and the SVG's CSS font comes from top-level `fontFamily`, not
  `themeVariables.fontFamily` (W010 otherwise). `assets/mermaid-theme.json` sets both. On Windows
  `command = "mmdc"` finds the npm `.cmd` shim. Remaining W010 from `mmdc`: a `&quot` fragment in
  sequence diagrams and `sans-serif` in gantt charts (generic fallbacks, harmless).
- **N11. 120-diagram run with `mmdc`** (`scratchpad/gen120.py`: 120 distinct diagrams over the nine
  types of N5, four per page; 84-page PDF). 0 errors; cold 25 to 27 s, warm 1 s (the cache lives in
  `<output>/.hideo-cache`, so a different `--output` is a cold build). The run found two things: the
  font check misread mermaid's `font-family: &quot;Libertinus Serif&quot;, ...` and warned wrongly
  (fixed in `font_problems`, with a test); and **class and ER diagrams were not reproducible**:
  `look: "neo"` adds random outline jitter, 13 of 120 SVGs differed between runs. The theme now sets
  `handDrawnSeed: 1`, after which two cold builds give identical PDFs. Remaining 13 W010 notices
  are gantt charts, where mermaid hardcodes `font-family="sans-serif"` on its text.
- **N6. Performance** (Windows 11, 32 logical cores, release build, `scripts/gen-corpus.py`:
  230 pages, 309 MB of JPEG heroes, 3 tiers, diagrams off): cold build **12.9 s**, warm build
  **2.3 s**, cold with `--jobs 1` 69.8 s (about 0.25 s per image, decode plus Lanczos3 plus encode).
  The goal (≤ 60 s cold, ≤ 10 s warm) is met with the default thread count, so `fast_image_resize`
  is not adopted. Each PDF is about 50 MB because the JPEGs are embedded unchanged.
- **N7. Test-plan deviations.** "Missing alt text is E006" cannot be provoked, because hideo always
  supplies alt text or marks an image as an artifact; the equivalent is `test_missing_glyph_is_e006…`
  (PDF/UA turns glyphs missing from the embedded fonts into errors; `check` warns first with the new
  W011). `lopdf` text extraction includes text inside `/Artifact` sequences, so the leader test inspects
  marked content (`text_ops`) instead of the extracted text. `image-compare` was dropped: the visual
  test counts differing pixels itself. The diagram fixtures use a stand-in script for `mmdc`; the CI
  fixture `full/` sets `diagrams.backend = "off"`.
- **N8. Added codes.** W009 (non-descriptive link text), W010 (notices: skipped tier, dropped code
  language, diagrams off, Typst warnings), W011 (characters not in the embedded fonts).
- **N9. Observation about Appendix B, not changed.** The spec asks for paragraph spacing 0.65 em
  and leading 0.75 em. In Typst both are gaps, so paragraphs are _closer_ together than the lines
  inside them and paragraphs are hardly separated. It is implemented as specified; confirm that this
  is the intended look.
- **N10. Not implemented.** `merman` evaluation, `--verbose` beyond a summary line, a check that a
  non-breaking table row really is not split, native-diagram contact sheet tooling.

> Items that depend on external systems (CI runs on Semaphore/AppVeyor, veraPDF, NVDA, `mmdc`
> on the real corpus, a release tag) are tracked here but cannot be completed from a local
> session; they are left unchecked on purpose.

---

> **Status of this document.** Revised 2026-11-17. Every crate version, API signature and
> platform claim below was checked on that date against crates.io, the published crate sources
> (`typst*` 0.15.1, `pulldown-cmark` 0.13.4/0.13.6, `mermaid-rs-renderer` 0.3.1, `lopdf` 0.47)
> and the Semaphore and AppVeyor documentation. Revision 3 (same day) switched Windows CI to AppVeyor and
> added accessibility (F24, 7.10). Items that could **not** be verified are marked
> **(verify)** and have an owner milestone. Appendix D lists what was wrong in the previous
> revision.

---

## 1. Executive Summary

`hideo` is a single-binary Rust tool that converts a folder of Markdown pages and images into one
PDF book per tier folder. It embeds the Typst compiler and PDF exporter in-process, so the only
runtime dependency is the binary itself (plus an optional external Mermaid renderer). The tool
prioritizes reproducibility, clear diagnostics, and strict adherence to a narrow Markdown profile.

**Key decisions in this revision:**

| Area                   | Decision                                                                                                                                                                                                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Typst integration      | `typst`, `typst-layout`, `typst-pdf`, `typst-kit` all pinned to `=0.15.1`. Use the **reworked** `typst-kit` (`FontStore`, `FileStore`, `FileLoader`), not the pre-0.15 `FontSearcher`.                                                                                                                 |
| Generated Typst layout | One virtual `.typ` file per page, pulled in by `#include`. Typst diagnostics then name the page directly; only the line needs mapping (see 7.9).                                                                                                                                                       |
| Fonts                  | `typst-kit` `embedded-fonts`: Libertinus Serif, New Computer Modern (+ Math), DejaVu Sans Mono. No sans-serif is embedded; the visual spec only needs the serif and the mono.                                                                                                                          |
| Markdown parser        | `pulldown-cmark` **0.13.6** (not 0.13.4). The "OffsetIter control-character panic" from the previous revision could not be substantiated and is dropped; defensive `catch_unwind` stays (see 5.1).                                                                                                     |
| Diagrams               | Backend is a command contract (`mmdc`-compatible CLI). `mmdr` (mermaid-rs-renderer) has the same `-i/-o/-c/-e` flags, so one config covers both. In-process `mmdr` and `merman` are candidates for the M7 gate. SVGs containing `<foreignObject>` are rejected because Typst renders them incorrectly. |
| CI                     | **Semaphore** for Linux (x86_64, ARM) and macOS (Apple Silicon); **AppVeyor** for Windows (Semaphore Cloud has no Windows agents). Two config files: `.semaphore/semaphore.yml` and `appveyor.yml` (8.6).                                                                                              |
| Release                | Plain `cargo build --release` + archive + checksum on Semaphore (Linux, macOS) and on AppVeyor (Windows). `cargo-dist` is dropped: its only CI backend is GitHub Actions.                                                                                                                              |
| Accessibility          | Tagged PDF is on by default and PDF/UA-1 validation is on by default (7.10). Table header rows are tagged as headers **only if** the emitter wraps them in `table.header(...)`; that is now a hard requirement (F16, F24).                                                                             |
| Repo state             | The repo is currently the leftover of the old Node.js project (see Appendix E for cleanup).                                                                                                                                                                                                            |

---

## 2. Goals, Success Criteria, and Non-Goals

### 2.1 Goal

One self-contained binary that turns a source folder of Markdown pages and images into one PDF book
per tier folder, with working internal links, a contents page, covers, hero images, diagrams and
styled tables.

### 2.2 Success Criteria

1. `cargo install --path .` or a downloaded binary is all a contributor needs (Rust ≥ 1.92, the
   MSRV of Typst 0.15.1).
2. Every requirement in section 4 is met and has a test.
3. A full build of ~230 pages and ~260 MB of images takes ≤ 60 seconds on a 2024-era laptop; warm
   rebuild ≤ 10 seconds.
4. A Markdown mistake produces an error naming the page and line.
5. Two builds of the same source are byte-identical.
6. The test suite passes on Linux (x86_64, aarch64), macOS (aarch64) and Windows (x86_64).
7. Every PDF is a tagged PDF that passes Typst's PDF/UA-1 validation; every table has a tagged header
   row (`TH` cells with column scope); every non-decorative image has alt text (7.10).

### 2.3 Non-Goals

- General Markdown support. Only the profile in section 5 is supported. Anything else is an error.
- A redesign of the visual look. Appendix B is the specification.
- Editing, serving, or previewing documents.
- Using Typst packages (`@preview/...`) or the network at build time. `typst-kit`'s package and
  downloader features stay **off**.

---

## 3. Architecture Decision: Embedded Typst Compiler

### 3.1 Decision

Parse Markdown with `pulldown-cmark`, emit Typst markup, compile in-process with the `typst` crate
(0.15.1), export PDF with `typst-pdf` (0.15.1).

### 3.2 Rationale

- Typst is a Rust project with a library API pattern: implement `typst::World`, call
  `typst::compile::<PagedDocument>(&world)`, export with `typst_pdf::pdf(&document, &options)`.
- Embedding avoids the external Typst CLI dependency.
- `typst-kit` 0.15 provides ready-made building blocks for exactly the parts we would otherwise
  hand-write: `FontStore` (`book()` / `font()` plug straight into `World`), `FileStore<L>`
  (source/file caching around a tiny `FileLoader` trait) and `fonts::embedded()`.
- No ready-made Markdown→Typst crate is worth adopting (a crates.io search found only generic
  document converters such as `shiva`). Pandoc and the Typst package `cmarker` are not embeddable
  in a strict-profile tool. The emitter is hand-written, which is also what makes strict errors
  with page and line possible.

### 3.3 Verified Facts and Consequences

| Fact (verified in 0.15.1 sources)                                                                                                                                                                    | Consequence                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `typst::compile<T: Output>(world: &dyn World) -> Warned<SourceResult<T>>`                                                                                                                            | Call as `compile::<PagedDocument>(&world)`. `PagedDocument` lives in `typst_layout`, which is **not** re-exported by `typst`, so add `typst-layout` as a dependency.                                                                                                                                                                                                       |
| `typst_pdf::pdf(&PagedDocument, &PdfOptions) -> SourceResult<Vec<u8>>`                                                                                                                               | Returns a `Result`, not bytes. Map its errors like compile errors.                                                                                                                                                                                                                                                                                                         |
| `PdfOptions { ident, creator, timestamp: Option<typst_pdf::Timestamp>, page_ranges, standards, tagged, pretty }`, `Default` available                                                                | The timestamp is Typst's own `Timestamp::new_utc(Datetime)`, **not** a `chrono` type. `tagged` defaults to `true` (adds accessibility tags; keep).                                                                                                                                                                                                                         |
| `Timestamp` is used only if the document's `date` is `auto`; with `timestamp: None` and `auto` no date is written at all. `ident: Smart::Auto` derives the PDF ID from title + author                | Reproducibility needs only a fixed timestamp (or none). The previous plan's "post-process the PDF to normalize dates" is not needed; the reproducibility test (F19) proves it.                                                                                                                                                                                             |
| `World::today(&self, offset: Option<typst::foundations::Duration>) -> Option<Datetime>`                                                                                                              | The previous plan had `Option<i64>`. Return a fixed date.                                                                                                                                                                                                                                                                                                                  |
| `FileId` is now `FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new("pages/a.typ")?))` (interned, `NonZeroU16`, so at most 65,535 distinct files per process)                        | The previous plan's `FileId::new(None, VirtualPath::new(..))` does not compile. `VirtualPath::new(&str)` returns a `Result`; use `VirtualPath::virtualize(root, os_path)` to turn OS paths into virtual ones (it handles Windows separators). Budget: ≥ 230 pages + images per process is far below the limit, but generate one `FileId` per distinct file, never per use. |
| `typst_kit::fonts::embedded()` yields Libertinus Serif, New Computer Modern, New Computer Modern Math, DejaVu Sans Mono (doc comment in `fonts.rs`)                                                  | Confirms the font claim. `FontSearcher` / `include_system_fonts` do not exist in 0.15.                                                                                                                                                                                                                                                                                     |
| `typst-kit` features are all off by default. Needed: `embedded-fonts`. **Not** needed: `datetime`, `emit-diagnostics`, `scan-fonts`, `system-*`, `universe-packages`, `system-downloader`, `watcher` | Smaller build, no network or OpenSSL.                                                                                                                                                                                                                                                                                                                                      |
| Typst warns on SVGs containing `<foreignObject>` ("might render incorrectly", typst/typst#1421)                                                                                                      | Gate diagram output (7.7).                                                                                                                                                                                                                                                                                                                                                 |
| SVG text is resolved through the `World`'s font book                                                                                                                                                 | SVG `font-family` must name an embedded font (7.7).                                                                                                                                                                                                                                                                                                                        |
| `typst`/`typst-kit` MSRV 1.92, edition 2024                                                                                                                                                          | `rust-version = "1.92"`, `edition = "2024"`.                                                                                                                                                                                                                                                                                                                               |

### 3.4 Version Pinning Strategy

```toml
[dependencies]
typst = "=0.15.1"
typst-layout = "=0.15.1" # PagedDocument
typst-pdf = "=0.15.1"
typst-kit = { version = "=0.15.1", features = ["embedded-fonts"] }
# typst-assets is pulled in transitively by `embedded-fonts`; do not depend on it directly.
# typst::syntax re-exports typst-syntax (FileId, Source, VirtualPath, ...).

[dev-dependencies]
typst-render = "=0.15.1" # render pages to PNG for visual snapshot tests
```

All Typst crates move in lock-step. Renovate must bump them as one group and **not** automerge
(see Appendix E). Upgrade only deliberately, with the full suite and the visual review passing.
Commit `Cargo.lock` (this is a binary crate).

Typst 0.15.1 was published 2026-07-17 (crates.io). The specific bug-fix list quoted in the previous
revision could not be verified and has been removed.

---

## 4. Functional Requirements

### 4.0 Requirements Index (reconstructed)

The original F1–F23 list is **not** in this file. The table below is reconstructed from the
milestone table, test names and Appendix B. **Confirm it against the original** before starting;
mismatches only affect labels, not design.

| ID  | Requirement                                                                            | Milestone |
| --- | -------------------------------------------------------------------------------------- | --------- |
| F1  | Tiers from config; one PDF per tier; empty tier skipped with warning                   | M1        |
| F2  | Page discovery (ignore prefixes at any depth, `.md` only)                              | M1        |
| F3  | Page order: folders and files, case-insensitive natural order                          | M1        |
| F4  | Frontmatter: `title`, `hero`, optional `subtitle`, `author`, `lang`, `hero_alt`        | M1        |
| F5  | Heading IDs: slug or explicit `{#id}`, unique per page                                 | M2        |
| F6  | Page labels, unique across the tier                                                    | M2        |
| F7  | Link rewriting (internal, cross-page, anchors, external, non-Markdown files)           | M2        |
| F8  | Unresolved-link report, grouped by PDF, `--deny-unresolved`                            | M2        |
| F9  | Markdown profile → Typst (paragraphs, emphasis, lists, quotes, code, rules, headings)  | M4        |
| F10 | Page head (hero pulled into margins, subtitle, author line)                            | M3/M6     |
| F11 | Hero image pipeline (resolve, scale, JPEG, cache, placeholder)                         | M3        |
| F12 | Cover (full-bleed, panel, tier line) and contents hero                                 | M3/M6     |
| F13 | Contents page (two columns, groups, dotted leaders, page numbers)                      | M6        |
| F14 | Footer page numbers on every page except the cover                                     | M6        |
| F15 | Diagrams (backend, cache, error format)                                                | M7        |
| F16 | Tables (width algorithm, font reduction, long-string breaks, repeating header)         | M5        |
| F17 | Lead-in paragraphs                                                                     | M4        |
| F18 | External-link marker (arrow icon)                                                      | M4        |
| F19 | Reproducible output (`SOURCE_DATE_EPOCH`)                                              | M6        |
| F20 | Diagnostics mapped to page and line                                                    | M6        |
| F21 | CLI: `build`, `check`, `list`, exit codes, `--help`                                    | M1/M9     |
| F22 | Configuration file + validation                                                        | M1        |
| F23 | Check mode (no PDF written, JSON report)                                               | M1/M9     |
| F24 | **(new)** Accessible PDF: tagged, PDF/UA-1, table headers, alt text, language, outline | M4/M5/M6  |

### 4.1 Clarifications and Additions

**F4 (Frontmatter).** The format is a deliberately tiny `key: value` subset, so it is hand-written
(~80 lines). Rejected alternatives: `gray_matter` (pulls a YAML engine and accepts far more than the
profile allows), `serde_yml`/`yaml-rust2` (full YAML contradicts "anything else is an error";
`serde_yaml` itself is archived).

- Strip a UTF-8 BOM; normalize `\r\n` and lone `\r` to `\n` **once, when reading the file**, so every
  later byte offset refers to the normalized text.
- Quoted values support `\"` inside double quotes and `\'` inside single quotes. Split on the _first_
  colon only. Duplicate keys: first wins. Indented lines are ignored.
- Error if `---` opens but never closes: report page and line 1.
- Record `body_line_offset` (number of frontmatter lines incl. both `---`) so body line numbers
  map to file line numbers by addition.

**F5 (Heading IDs).**

- Slug: lowercase; keep `char::is_alphanumeric()`, `_`, `-`; spaces become hyphens, runs of spaces do
  **not** collapse; other punctuation is dropped. Empty result → `section`. Duplicates get `-1`,
  `-2`, … in document order.
- Explicit `{#id}` (needs `Options::ENABLE_HEADING_ATTRIBUTES`, see section 5) is used verbatim but
  must match `^[a-zA-Z][a-zA-Z0-9_-]*$`; otherwise error naming page and line.
- **Typst labels are global to the document**, so two pages with `## Setup` would collide. Emit the
  label as `<{page_key}--{id}>` where `page_key` is the slug of the page's path relative to the tier
  folder (e.g. `required/01_intro.md` → `required-01-intro`). The page's own anchor is
  `<{page_key}>`. Keep a `HashMap<(page_key, id), Label>`; the emitter and link rewriter both go
  through it.

**F7 (Link rewriting).**

- External if the destination starts with a scheme matching `^[A-Za-z][A-Za-z0-9+.-]+:` (two or more
  scheme characters, so `C:\x` is not a scheme) or `//`. Also `mailto:`. These become
  `#link("url")[…]` and get the arrow marker (F18) unless the scheme is `mailto`.
- Percent-decode (`%20`) the path part before resolution (`percent-encoding` crate).
- Resolve relative to the current page's directory, then normalize `.` and `..` **lexically** (no
  filesystem access, no `canonicalize`: it fails on missing files and yields `\\?\` paths on Windows).
- Directories and non-`.md` files → left as an ordinary link to the original path (not part of the PDF),
  reported only if the file does not exist.
- Fragment-only (`#section`): resolve in the current page.
- Case: compare resolved paths exactly against the discovered page set (a `HashMap` of normalized
  tier-relative paths). Additionally build a lower-cased map; if the exact lookup fails but the
  lower-cased one succeeds, resolve it **and** warn "case differs from file on disk" on every platform. This gives identical behaviour on Windows,
  macOS and Linux, which is what makes the Windows CI meaningful.
- Links that leave the tier: marked in the report as "outside this PDF" and rendered as plain text
  with the link target in the report only. **(Decision D5: confirm.)**

**F10 (Page head, hero pulled into margins).** Do not rely on `place` offsets. Use
`#pad(x: -margin-x, top: -margin-y)[#image("…", width: 100%)]` or a `#block(width: 100% + 2*margin-x)`
moved with `#move(dx: -margin-x)`. Whichever is chosen, the hero must not push the first heading
down by more than the 5 mm spec. Prove it in M3 with a `typst-render` snapshot.

**F13 (Contents).** Entries need page numbers that are known only after layout. Use
`context` + `locate(<page_key>)` and `counter(page).at(loc)` in the template; Typst iterates layout
until introspection converges (the compiler's `MAX_ITERS`), so no two-pass build is needed. Dotted
leader: `box(width: 1fr, repeat[.])`. Cover has `set page(numbering: none)`; the page counter starts
at 1 on the contents page (**decision D6**: or on the cover?).

**F15 (Diagrams, error format).**

```
error: diagram failed to render
  page: required/01_intro.md
  line: 47
  first line: graph TD; A-->B
  backend: external (mmdc)
  reason: <stderr, last 10 lines>
```

First line truncated to 60 characters. Line is the Markdown line of the opening fence.

**F19 (Reproducibility).**

- `SOURCE_DATE_EPOCH` (seconds, UTC). If set and invalid → exit 2. If unset use `946684800`
  (2000-01-01T00:00:00Z).
- Convert with `chrono::DateTime::from_timestamp(secs, 0)` (add `chrono` with
  `default-features = false, features = ["std"]`), then
  `typst::foundations::Datetime::from_ymd_hms(y, m, d, h, mi, s)` and
  `typst_pdf::Timestamp::new_utc(dt)`.
- The same instant is returned from `World::today`.
- `ident: Smart::Auto` and `creator: Smart::Custom(Some(format!("hideo {}", env!("CARGO_PKG_VERSION"))))`.
- Everything iterated in the pipeline must be ordered (sort `read_dir`/`walkdir` output, use
  `BTreeMap`, never iterate a `HashMap` into output). Rayon work must be collected in input order.
- Test F19 must run the two builds **in separate processes with different `--jobs`** (1 and 8) and
  compare bytes.

**F23 (Check mode).** Also validate: every `hero` exists and is readable; every cover pattern
resolves (or warn about the placeholder); diagram sources non-empty; heading IDs unique per page;
`title` and `hero` present. `--format json` prints
`{"errors":[…],"warnings":[…],"stats":{…}}` with each item
`{"code","page","line","message"}`. Define the codes now (E001 frontmatter, E002 unsupported construct,
E003 bad heading id, E004 diagram, E005 compile, W001 unresolved link, W002 inline HTML, W003 missing
hero, W004 table overflow, W005 case mismatch, W006 empty table header cell, W007 skipped heading level,
W008 missing diagram alt text, E006 PDF/UA validation failure) so tests can assert on them.

---

## 5. Markdown Profile (Clarified)

| Construct          | Clarification                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parser options     | Start from `Options::empty()` (CommonMark). **Enable** `ENABLE_TABLES` and `ENABLE_HEADING_ATTRIBUTES` (needed for `{#id}`; without it `{#id}` is literal text). Enable `ENABLE_STRIKETHROUGH`, `ENABLE_FOOTNOTES`, `ENABLE_TASKLISTS`, `ENABLE_DEFINITION_LIST`, `ENABLE_SUPERSCRIPT`, `ENABLE_SUBSCRIPT` **only to detect them** and raise an error naming page and line. (The previous plan said pulldown-cmark "parses some of these by default"; it does not: every extension is opt-in.)             |
| Dollar signs       | Always literal. Never enable `ENABLE_MATH`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Metadata blocks    | Do not enable `ENABLE_*_METADATA_BLOCKS`; frontmatter is stripped by hideo's own parser first. A second `---` block in the body is just a thematic break.                                                                                                                                                                                                                                                                                                                                                  |
| Inline HTML        | `Event::InlineHtml` → warning, text dropped; `Event::Html` inside `Tag::HtmlBlock` → error.                                                                                                                                                                                                                                                                                                                                                                                                                |
| HTML comments      | A whole block matching `<!-- … -->` is dropped silently.                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Smart punctuation  | Do **not** enable `ENABLE_SMART_PUNCTUATION`. Typst's markup already converts `...` → `…`, `--` → en dash, `---` → em dash, `-?` → soft hyphen and applies smart quotes, so **no manual `...` replacement is needed** (the previous plan asked for one). The escape table (7.5) therefore must escape these sequences when they occur literally in code spans and raw blocks, and must decide whether literal `--` in prose is allowed to become an en dash (recommended: yes, same as smart punctuation). |
| Images in the body | `Tag::Image` is not specified anywhere in the original requirements. **Decision D4.** Until decided: error "body images are not supported".                                                                                                                                                                                                                                                                                                                                                                |
| Line numbers       | Use `Parser::new_ext(body, opts).into_offset_iter()`; build a `Vec<usize>` of line starts once per page and `binary_search` each event's byte offset. Line = `body_line_offset + index + 1`.                                                                                                                                                                                                                                                                                                               |

### 5.1 pulldown-cmark Version and Known Issues

**Use `pulldown-cmark = { version = "0.13.6", default-features = false }`** (the default features
`getopts` and `html` are for the bundled CLI and HTML writer; hideo needs neither). MSRV 1.71.

What the release notes actually say (GitHub releases, checked 2026-11-17):

- 0.13.4: "Fix `panic` in specific cases with `TightParagraph`."
- 0.13.5: backports "security issues" fixed on `main`; the backported commits concern **quadratic
  parse time** (a line of `*a_`; `[^` on every line with footnotes enabled; many blank lines) and a
  **panic with definition-list offsets inside a combining character**.
- 0.13.6: a separate advisory about wikilink DoS.

The "OffsetIter panics on NUL / VT / FF in link reference definitions" claim from the previous
revision **has no source** and is removed. The real risk is smaller and different: a hostile or
unlucky input can be slow or panic. Mitigations, all cheap:

1. Use 0.13.6 or later; monitor releases (Renovate).
2. Wrap the parse + event walk of each page in `std::panic::catch_unwind`; report E005-style error
   naming the page. Set `panic = "unwind"` (the default; do **not** use `panic = "abort"` in the
   release profile or `catch_unwind` is useless).
3. Strip NUL only (it is invalid in a text file and CommonMark replaces it anyway). Do not strip VT/FF
   without a documented reason.
4. Keep a `tests/fixtures/edge/pathological.md` containing the three quadratic patterns above and
   assert the page parses in < 1 s.

---

## 6. Crates (Revised)

Versions are the latest stable on crates.io on 2026-11-17.

| Crate                                             | Version          | Use                              | Note                                                                                                                                                                                                                     |
| ------------------------------------------------- | ---------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `clap`                                            | 4.6              | CLI                              | `derive` feature                                                                                                                                                                                                         |
| `serde`                                           | 1                | Config, reports                  | `derive` feature (no separate `serde_derive` dependency)                                                                                                                                                                 |
| `toml`                                            | **1.2**          | Config file                      | The previous plan said 0.8; 1.x is current (TOML 1.1). `deny_unknown_fields` on every config struct satisfies `test_config_unknown_key_errors`                                                                           |
| `serde_json`                                      | 1                | JSON output                      |                                                                                                                                                                                                                          |
| `pulldown-cmark`                                  | **0.13.6**       | Markdown parsing                 | `default-features = false`; see 5.1                                                                                                                                                                                      |
| `typst`, `typst-layout`, `typst-pdf`, `typst-kit` | **=0.15.1**      | Compiler                         | See 3.4                                                                                                                                                                                                                  |
| `image`                                           | 0.25 (≥ 0.25.10) | Hero decode/resize/encode        | `default-features = false, features = ["jpeg", "png"]` (add `webp` only if sources use it)                                                                                                                               |
| `camino`                                          | 1.2              | UTF-8 paths (`Utf8PathBuf`)      | Typst only takes `&str` virtual paths; reject non-UTF-8 file names once at discovery instead of everywhere                                                                                                               |
| `percent-encoding`                                | 2.3              | Decode `%20` in links            |                                                                                                                                                                                                                          |
| `walkdir`                                         | 2.5              | Page discovery                   | Stable but unchanged since 2024; fine                                                                                                                                                                                    |
| `alphanumeric-sort`                               | 1.5              | Natural ordering                 | **(verify)** it ignores case as required by `test_page_order_case_insensitive_natural`; otherwise compare `to_lowercase()` keys with a hand-written ~30-line natural comparator. Do not use `natord` (last release 2015) |
| `rayon`                                           | 1.12             | Parallel image/diagram work      | Collect in input order (F19)                                                                                                                                                                                             |
| `blake3`                                          | 1.8              | Cache keys                       |                                                                                                                                                                                                                          |
| `anyhow`, `thiserror`                             | 1 / 2            | Errors                           | `thiserror` for the typed page+line error, `anyhow` only in `main`                                                                                                                                                       |
| `chrono`                                          | 0.4              | `SOURCE_DATE_EPOCH` → civil date | `default-features = false, features = ["std"]`. Could be replaced by a 12-line civil-from-days function to save a dependency                                                                                             |
| `tempfile`                                        | 3.27             | External diagram scratch         |                                                                                                                                                                                                                          |
| `wait-timeout`                                    | 0.2              | 30 s per-diagram timeout         | `std::process::Child` has no timeout; this is the small, boring answer                                                                                                                                                   |
| `tracing` + `tracing-subscriber`                  | 0.1 / 0.3        | Logging                          | Optional. `-v` can be served by plain `eprintln!` until M8                                                                                                                                                               |
| **dev:** `assert_cmd`                             | 2.2              | CLI tests                        |                                                                                                                                                                                                                          |
| **dev:** `predicates`                             | 3.1              | CLI assertions                   |                                                                                                                                                                                                                          |
| **dev:** `insta`                                  | 1.49             | Snapshot tests                   |                                                                                                                                                                                                                          |
| **dev:** `proptest`                               | 1.12             | Property tests                   |                                                                                                                                                                                                                          |
| **dev:** `lopdf`                                  | **0.47**         | PDF inspection                   | MSRV 1.88. `Document::load_mem`, `get_pages()`, `get_page_annotations(page_id)` and `extract_text(&[page_numbers])` all exist in 0.47 (the previous plan said 0.44)                                                      |
| **dev:** `typst-render`                           | =0.15.1          | Render pages to PNG              | Replaces the `pdftoppm` dependency in the visual review (8.5) and lets snapshots run on every OS                                                                                                                         |
| **dev:** `image-compare`                          | 0.5              | Tolerant image diff              | Optional, for PNG snapshots across OSes (font hinting is not an issue because fonts are embedded, but allow ≤ 0.1 % pixels)                                                                                              |

### 6.1 Considered but Deferred or Rejected

| Crate                                                          | Verdict                                                                                                                                                                                                                                                         |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fast_image_resize` 6.1                                        | SIMD resize; adopt at M8 **if** the benchmark shows `imageops::resize(Lanczos3)` dominates the 60 s budget. The 260 MB of images is the likely hot spot, so benchmark in M3 already.                                                                            |
| `jpeg-rusturbo` 0.9                                            | Exists (SIMD encoder, Jun 2026) but is small (~10 k downloads). The "~3×" claim in the previous plan is unverified. `jpeg-encoder` 0.7 (pure Rust, widely used) is the safer alternative. Defer until measured.                                                 |
| `comrak` 1.0                                                   | Full AST parser. `pulldown-cmark`'s event stream is enough; revisit only if table handling (M5) becomes unwieldy.                                                                                                                                               |
| `gray_matter`, `serde_yml`, `yaml-rust2`                       | Too permissive for F4; see 4.1.                                                                                                                                                                                                                                 |
| `codespan-reporting`, `ariadne`, `miette`, `annotate-snippets` | `typst_kit::diagnostics::emit` uses codespan 0.11 and would print generated-Typst excerpts, which are meaningless to authors. Print the plain format shown in F15 instead (no dependency). Revisit only if authors ask for source excerpts.                     |
| `url`                                                          | Not needed: scheme detection is a 3-line regex-free check (F7).                                                                                                                                                                                                 |
| `cargo-dist` 0.34 (binary `dist`)                              | Rejected: its only CI backend is GitHub Actions. Releases are built by a Semaphore promotion (8.7).                                                                                                                                                             |
| `mermaid-rs-renderer` 0.3.1 (`mmdr`)                           | Candidate native backend, MIT, pure Rust. Own `Theme` struct plus a Mermaid-style config-file loader (`config::load_config`). README: "may not yet match mermaid-cli in all cases". `default-features = false` drops the CLI and `png` (resvg) features.        |
| `merman` 0.10                                                  | Second candidate native backend (MIT OR Apache-2.0, MSRV 1.87), "parity-focused", targets Mermaid 12.3. Newer and more active than `mmdr`. Docs coverage is thin; no confirmed `themeVariables` support, no stated `<foreignObject>` behaviour. Evaluate in M7. |
| `pdf-extract`, `pdfium-render`, `hayro`                        | Alternatives to `lopdf` for PDF text/rendering in tests. `lopdf` + `typst-render` is enough and has no native dependency.                                                                                                                                       |

---

## 7. Design

### 7.1 Crate Layout

```
hideo/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml         # channel = "stable", components = rustfmt, clippy
  hideo.toml                  # example config
  assets/
    template.typ              # page styles, helpers (include_str!)
    arrow.svg                 # external-link icon (Appendix B)
    mermaid-theme.json        # default diagram theme (Appendix C)
  src/
    main.rs                   # entry point, exit codes
    cli.rs                    # clap definitions
    config.rs                 # serde structs, defaults, validation
    error.rs                  # HideoError { code, page, line, message }, Warning
    discover.rs               # find and order pages (F2, F3)
    frontmatter.rs            # F4 parser
    page.rs                   # Page struct, line index, heading extraction
    linkmap.rs                # F5–F8 (labels, rewriting, report)
    markdown/
      mod.rs                  # parse, event walk, unsupported-construct detection
      emit.rs                 # events → Typst markup + LineMap
      escape.rs               # Typst escaping
      tables.rs               # F16
      lead.rs                 # F17
    images.rs                 # heroes, covers, cache
    diagrams/
      mod.rs                  # DiagramRenderer trait, cache, foreignObject check
      native.rs               # in-process backend (feature "native-diagrams")
      external.rs             # runs external command
    template.rs               # fills tokens, builds cover/contents/page heads
    world.rs                  # typst::World implementation
    compile.rs                # compile, export, diagnostic mapping
    report.rs                 # unresolved links, warnings, timings
  tests/
    fixtures/                 # small books covering every construct
    integration/              # assert_cmd + lopdf tests
  .semaphore/
    semaphore.yml             # CI for Linux and macOS (8.6)
    release.yml               # release promotion (8.7)
  appveyor.yml                # CI and release archive for Windows (8.6)
```

`Cargo.toml` additions beyond dependencies:

```toml
[package]
name = "hideo"
edition = "2024"
rust-version = "1.92"

[features]
default = []
native-diagrams = ["dep:mermaid-rs-renderer"] # or merman, decided at the M7 gate

[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3 # Typst in an unoptimized build is very slow; the proptests compile thousands of documents

[profile.release]
lto = "thin"
```

### 7.2 Data Flow for One Tier

1. **Discover and order** pages (F2, F3). Reject non-UTF-8 paths here. Read each file once. Normalize
   BOM and CRLF.
2. **Strip NUL**, then **parse frontmatter** (F4). Validate `title` and `hero`.
3. **Parse body** with `pulldown-cmark` (inside `catch_unwind`), collecting events. Detect unsupported
   constructs and emit errors.
4. **Compute heading IDs** and the tier label map (F5, F6).
5. **Rewrite links** (F7). Record unresolved entries.
6. **Emit Typst markup** per page, recording a `LineMap` (7.9).
7. **Prepare images in parallel:** heroes, cover, contents hero (F11, F12).
8. **Render diagrams in parallel**, deduplicated by hash (F15).
9. **Assemble the virtual file tree:** `main.typ` (template, cover, contents, one `#include` per page),
   `pages/NNN.typ` (page head + body), `images/…`, `diagrams/….svg`.
10. **Compile and export** with the fixed timestamp. Write `{tier}.pdf` atomically (write
    `{tier}.pdf.tmp`, then rename) so an error never leaves a half-written PDF.
11. **Map diagnostics** back to page files and Markdown lines.
12. **Print warnings** and the unresolved-link report.

Tiers are independent and may build in parallel (`rayon`), but share one immutable `FontStore`
(`static LazyLock<FontStore>`) and the interned `FileId` table (global, thread-safe).

### 7.3 Command Line

```
hideo build [TIER...]
    -i, --input <DIR>           Source folder (default: .)
    -o, --output <DIR>          Output folder (default: pdf-output)
    -c, --config <FILE>         Config file (default: hideo.toml in source folder)
        --diagrams <BACKEND>    native | external | off (default: external)
        --keep-typst <DIR>      Write generated Typst source per tier
        --deny-unresolved       Exit 1 if any internal link is unresolved
        --jobs <N>              Worker threads (default: logical cores)
    -q, --quiet                 Suppress warnings
    -v, --verbose               Verbose logging

hideo check [TIER...]
        --format <text|json>    Report format (default: text)

hideo list [TIER...]
        --format <text|json>    Print pages in PDF order
```

**Exit codes:** `0` success; `1` build or content error; `2` usage or configuration error (bad CLI
args, invalid TOML, invalid `SOURCE_DATE_EPOCH`). clap exits with 2 by default on usage errors, which
agrees.

If `--diagrams native` is requested but the binary was built without `native-diagrams`, exit 2 with a
message naming the feature.

### 7.4 Configuration

The TOML structure from Appendix A is retained. Validation rules (config.rs):

- `tiers` non-empty, unique, and each a valid folder name (no path separators).
- `page.width_mm` > `2 * page.margin_x_mm`.
- `images.jpeg_quality` 1–100; `images.dpi` 72–600.
- `diagrams.backend` ∈ `native`, `external`, `off`.
- If `backend = "external"`: `diagrams.command` non-empty. `diagrams.args` (optional) defaults to the
  `mmdc`/`mmdr`-compatible template
  `["-i", "{input}", "-o", "{output}", "-c", "{theme}", "-e", "svg"]`; placeholders are substituted
  per invocation. A configurable argument list replaces the previous fixed contract, which named a
  non-existent binary (`mermaid-renderer`).
- Paths in the config are relative to the config file, not the CWD.
- `#[serde(deny_unknown_fields)]` everywhere.
- `book.lang` is **required** (decision D11: no default, so the language is never silently English). It must
  be a valid BCP 47 tag accepted by Typst's `text(lang: …)` (two or three letters, optional region). A missing
  `[book]` section or `lang` key is a configuration error (exit 2) naming the key. It is the **document**
  language (7.10); `lang` in a page's frontmatter overrides it for that page's body only.
- `accessibility.pdf_ua` (bool, default `true`): enforce PDF/UA-1 validation. `accessibility.tagged`
  (bool, default `true`): write tags. `pdf_ua = true` with `tagged = false` is a configuration error (exit 2).

### 7.5 Markdown to Typst Emission

#### Escaping (escape.rs)

The highest-risk module. Two designs were considered:

- **A (recommended, keeps smart quotes):** emit text as Typst markup and escape it with the table
  below.
- **B (smaller attack surface):** emit text as string literals, `#"…"`, escaping only `\` and `"`.
  Typst's markup parser then never sees the text, so there is nothing to forget. Cost: no smart
  quotes and no `--`/`...` conversion, so these would have to be done by hand.

Choose A unless the M4 property test keeps finding holes; B is the escape hatch and can be
swapped in behind the same `fn escape_text(&str, Position) -> String`.

**Text mode escaping:**

| Character / sequence                                 | Escape as                                      | Reason                                                       |
| ---------------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------ |
| `\`                                                  | `\\`                                           | Typst escape                                                 |
| `#`                                                  | `\#`                                           | Markup/script                                                |
| `$`                                                  | `\$`                                           | Math                                                         |
| `@`                                                  | `\@`                                           | Reference                                                    |
| `*`                                                  | `\*`                                           | Strong                                                       |
| `_`                                                  | `\_`                                           | Emphasis                                                     |
| `` ` ``                                              | `` \` ``                                       | Raw                                                          |
| `<`                                                  | `\<`                                           | Label                                                        |
| `>`                                                  | `\>`                                           | Label end (harmless, kept for symmetry)                      |
| `[` `]`                                              | `\[` `\]`                                      | Content block                                                |
| `~`                                                  | `\~`                                           | Non-breaking space                                           |
| `//`                                                 | `\/\/`                                         | Line comment; also stops bare-URL auto-linking (`https://…`) |
| `/*` and `*/`                                        | `\/*` and `*\/`                                | Block comment                                                |
| `=`, `-`, `+`, `/` at line start; `N.` at line start | `\=` `\-` `\+` `\/` `\N.`                      | Heading, list, enum, term list                               |
| `'` `"`                                              | unchanged                                      | Intended: Typst smart quotes                                 |
| `...`, `--`, `---`, `-?`                             | unchanged in prose (intended), escaped in code | Typst shorthands                                             |
| U+200B (zero-width space)                            | unchanged (never escaped)                      | Inserted by tables.rs                                        |
| `\n` inside a paragraph                              | `\n`                                           | soft break is a space                                        |
| Hard break                                           | `\` + newline                                  |                                                              |

**Raw blocks and inline code:** use a raw delimiter with more backticks than the longest backtick
run in the content (minimum 3 for blocks, 1 for inline; if the inline content starts or ends with a
backtick, pad with a space). Language tag: pass the info string through only if it matches
`^[A-Za-z0-9_+#.-]+$`, otherwise drop it with a warning. Typst highlights raw blocks using its
built-in syntax set; unknown tags are not an error.

**Property test** (corrected API; the previous snippet did not compile):

```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]  // 10 000 is too slow for CI; run 10 000 nightly
    #[test]
    fn emitted_text_compiles_and_survives(s in "\\PC{0,200}") {
        let src = format!("#set page(width: 200pt, height: auto)\n{}", escape_text(&s, Position::Inline));
        let world = TestWorld::new(src);
        let warned = typst::compile::<PagedDocument>(&world);
        prop_assert!(warned.output.is_ok(), "{:?}", warned.output.err());
        prop_assert!(warned.warnings.is_empty(), "{:?}", warned.warnings);
    }
}
```

Strengthen it by also asserting that the extracted text of the first frame equals `s` with Typst's
shorthand conversions applied (walk `FrameItem::Text`), otherwise silent mis-escapes (e.g. `*` eaten)
pass. Use `proptest` regression files and commit them.

#### Tables (tables.rs)

- **Text width:** `page_width_mm - 2 * margin_x_mm` converted to points (1 mm = 2.834645669 pt).
- **Widths:** compute preferred and minimum width per column from measured string widths. Measure with
  an approximation (average glyph width of Libertinus at the table font size, 0.5 em; **(verify)**
  against the visual fixtures) or, exactly, by compiling a one-off measuring document
  (`measure()` in Typst). Start with the approximation; the hand-computed fixtures (level 4) pin it.
- **Fractions:** normalize to fractions summing to 1.0 and emit `columns: (0.3fr, 0.7fr, …)`.
- **Font sizes:** try 8.5, 7.5, 7.0 pt; first size where `sum(min_widths) <= text_width` wins. If none
  fits use 7.0 pt, allow overflow, warn W004.
- **Long strings:** insert U+200B after `.`, `/`, `_`, `-`, `?`, `=`, `&` in whitespace-delimited
  tokens of length ≥ 16. Do this **before** escaping; the ZWSP is not escaped.
- Alignment from `Tag::Table(alignments)` → `align: (left, center, right, …)`.
- **Header row (accessibility-critical).** Every Markdown table has a header row (the parser emits
  `Tag::TableHead`). Emit exactly that row as `table.header([…], […], …)` and all other rows as plain
  cells. A plain first row is tagged `TD` (data) in the PDF, **not** `TH`, whatever its styling; bold text
  and a fill colour do not make a header. Verified in `typst-pdf` 0.15.1: cells inside `table.header` get
  the kind `Header(level, Column)` and are written as `TH` with a column scope; when every row is uniform
  and the table has both a header and a body, rows are grouped into `THead`/`TBody` (7.10).
  `table.header` repeats across pages by default (`repeat: true`); the repeated copies are marked as
  artifacts automatically, so screen readers do not hear the header twice (Typst docs).
- Markdown cannot express row headers, so the first column stays `TD`. That is acceptable: scope is
  `Column` for all tables.
- Empty header cells are an accessibility defect: warn W006 "empty table header cell" (page and line).
- Rows that fit are kept intact with `table(breakable: …)`/`block(breakable: false)` per row as Appendix B
  requires. **(verify)** the exact Typst 0.15 incantation for non-splitting rows in M5 with a two-page
  table fixture, and verify that it does not break the `THead`/`TBody` grouping (test in 8.2).
- Do not use `grid` for any Markdown table (a grid is announced as plain content, not as a table).
- ZWSP (U+200B) inserted for long-string breaking is harmless to screen readers in current engines but
  appears in copied text; keep it, strip it in the test that compares extracted text, and note it in the
  user docs **(verify)** with one screen reader pass in M8.

#### Lead-in paragraphs (lead.rs)

Detection: a paragraph whose first event is `Start(Strong)` followed by **exactly one** `Text` event
and `End(Strong)`, where the text ends in `.` or `:` and is ≤ 40 characters (a 40-character text ending
in `.` qualifies). Emission: `#lead[Strong text]` and then the rest of the paragraph. `**A** **B.**` is
two separate strong runs and does not qualify. Strong text containing code or links does not qualify.

### 7.6 Images (images.rs)

**Hero pipeline:**

1. Resolve path: next to the page first, then relative to the source folder.
2. Read bytes; decode with `image::load_from_memory`. Cap decoded dimensions (e.g. 20 000 × 20 000)
   via `image::ImageReader` limits to prevent decompression bombs.
3. Honour EXIF orientation **(verify)**: decide whether to apply it. Most camera JPEGs in a 260 MB
   corpus carry it. `image` 0.25 can read it (`ImageDecoder::orientation`) and apply it
   (`DynamicImage::apply_orientation`).
4. Convert to RGB8; composite alpha onto white.
5. Target width `ceil(page_width_mm * dpi / 25.4)`. If wider, resize with Lanczos3 (never upscale).
6. Encode JPEG at the configured quality; keep the original aspect ratio.
7. Register as a virtual file `images/<hash>.jpg`. Typst embeds JPEGs without re-encoding.

**Cache key:** `blake3(file_bytes ‖ canonical settings JSON ‖ hideo_image_pipeline_version)`. Stored at
`<cache_dir>/images/<hash>.jpg`; write via temp file + rename so parallel runs cannot read a partial
file. `cache_dir` is relative to the output folder (the previous Appendix A example had it relative
to the CWD; pick one: **output folder**).

**Placeholder (missing hero):** 16:9 light grey (`#E7E3EF`) JPEG generated in memory; warning W003.
No disk cache.

**Covers:** `cover_pattern = "images/cover_front_{tier}.png"`; `{tier}` is the tier folder name. Missing →
grey-violet placeholder (Appendix B).

### 7.7 Diagrams (diagrams/mod.rs)

```rust
pub trait DiagramRenderer: Send + Sync {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError>; // SVG
    fn name(&self) -> &'static str;
}
```

**Common post-conditions (enforced in `mod.rs`, for every backend):**

1. Output starts with `<svg` (after optional XML prolog).
2. Output must **not** contain `<foreignObject`. Typst warns that such SVGs render incorrectly (labels
   typically vanish). Treat as a render failure (E004) with the hint "set htmlLabels to false".
3. Every `font-family` in the SVG must resolve to an embedded font. Typst resolves SVG text through its
   own font book, so set `"fontFamily": "Libertinus Serif"` in the theme (or `DejaVu Sans Mono`).
   Otherwise labels silently fall back to another face and may overflow their boxes.
4. Cache key: `blake3(source ‖ theme_json ‖ backend_name ‖ backend_version)`; cache stored as
   `<cache_dir>/diagrams/<hash>.svg`.

**External backend:**

- Spawn `command` with `args` placeholders substituted (`{input}` temp `.mmd`, `{output}` temp `.svg`,
  `{theme}` path to the written theme JSON). Pipe stdout and stderr, wait with `wait-timeout` for 30 s,
  kill on timeout, include the last 10 stderr lines in the error.
- `mmdc` (mermaid-cli) needs Node and a headless Chromium. Its flowchart default is HTML labels
  (`<foreignObject>`), so the default theme (Appendix C) sets `"flowchart": {"htmlLabels": false}`.
  Other diagram types may still emit `foreignObject`; the post-condition catches it. **(verify)** with
  the real content at M7.
- `mmdr` accepts the same `-i`, `-o`, `-c configFile`, `-e svg` flags (verified in its CLI source), so
  `command = "mmdr"` is a drop-in with no extra code.
- Spawn at most `--jobs` processes; mmdc start-up is ~2 s each (published benchmark), so with ~120 diagrams
  caching matters more than anything else: cold build is bounded by cache misses, warm build is free.

**Native backend (feature `native-diagrams`):**

- `mermaid-rs-renderer` 0.3.1 (`default-features = false`): `config::load_config(Some(theme_path))`
  returns `Config { theme, layout, render }`; then
  `mermaid_rs_renderer::render_with_options(src, RenderOptions { theme: cfg.theme, layout: cfg.layout })`
  → SVG `String`. Use `render_strict` / `parse_mermaid_strict` to turn unsupported syntax into a
  `ParseError` instead of a silently wrong picture.
- `merman` 0.10: `Renderer::new().render(RenderRequest::svg(src, OperationControl::new(), SvgRequest::default()))`
  → `RenderOutput::Svg(Some(svg))`, `svg.svg()` is the string. Needs Rust 1.95 per its docs page
  (its manifest says 1.87; use the stricter **(verify)**).
- **Font mismatch risk:** native renderers measure text with their own font database or fallback
  widths while Typst draws with the embedded fonts, so boxes can be too tight. The M7 contact sheet
  must show this, and the check must be done at the final PDF scale.

**M7 Gate protocol (automated where possible):**

1. Extract all Mermaid blocks from the target content (~120) into `m7/diagrams/NNN.mmd`.
2. Render each with: external (mmdc), `mmdr`, `merman`.
3. Machine checks per output: renders without error; no `<foreignObject`; all fonts embedded; width/height
   present. Record pass/fail in a CSV.
4. Compile each SVG into a one-page Typst document at the final column width with `typst-render` and
   build a contact sheet (grid PNG, labelled by backend and diagram id).
5. Manual review: any diagram that fails or looks visibly worse than the external output disqualifies
   that backend **for that diagram type**. Allowed outcome: native for the types that pass, external
   fallback for the rest (`backend = "native"` + `fallback_command`), not an all-or-nothing choice.
6. If nothing passes: ship with the external backend as default and document "standalone except for
   diagrams".

### 7.8 Typst World (world.rs)

Implemented with the reworked `typst-kit` (verified against 0.15.1 sources):

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::files::{FileLoader, FileStore};
use typst_kit::fonts::{self, FontStore};

static LIBRARY: LazyLock<LazyHash<Library>> =
    LazyLock::new(|| LazyHash::new(Library::default()));

static FONTS: LazyLock<FontStore> = LazyLock::new(|| {
    let mut store = FontStore::new();
    store.extend(fonts::embedded());          // needs the `embedded-fonts` feature
    store
});

/// All inputs live in memory: generated .typ pages, JPEGs, SVGs.
struct MemLoader { files: HashMap<FileId, Bytes> }

impl FileLoader for MemLoader {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or_else(|| {
            FileError::NotFound(PathBuf::from(id.vpath().get_without_slash()))
        })
    }
}

pub struct HideoWorld {
    main: FileId,
    files: FileStore<MemLoader>,
    today: Datetime,            // fixed (F19)
}

impl World for HideoWorld {
    fn library(&self) -> &LazyHash<Library> { &LIBRARY }
    fn book(&self) -> &LazyHash<FontBook> { FONTS.book() }
    fn main(&self) -> FileId { self.main }
    fn source(&self, id: FileId) -> FileResult<Source> { self.files.source(id) }
    fn file(&self, id: FileId) -> FileResult<Bytes> { self.files.file(id) }
    fn font(&self, index: usize) -> Option<Font> { FONTS.font(index) }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> { Some(self.today) }
}
```

Notes:

- `World: Send + Sync`; `FileStore` uses an internal mutex, so this is fine.
- Create ids with
  `FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new("pages/001.typ")?))`.
  `main` is `main.typ`.
- Virtual paths use forward slashes and are rooted; build them from OS paths with
  `VirtualPath::virtualize(root, path)`, from tier-relative strings with `VirtualPath::new`. Add a unit test
  that feeds a `C:\…\a\b.md`-style path on every OS.
- **Fonts:** only the embedded four families exist (no system fonts, so output cannot vary with the
  machine). Glyphs not covered by Libertinus/DejaVu (e.g. CJK) render as missing-glyph boxes: add a
  check in `check` mode that warns when a page contains characters outside the embedded coverage
  **(verify)** feasibility with `FontBook`/`Font::ttf().glyph_index`.
- The generated Typst is assembled as virtual files, so no generated Typst touches disk except with
  `--keep-typst`.

### 7.9 Compile, Export and Diagnostics (compile.rs)

```rust
use typst::diag::Warned;
use typst::foundations::{Datetime, Smart};
use typst_layout::PagedDocument;
use typst_pdf::{PdfOptions, Timestamp};

let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
let document = output.map_err(|errs| map_diagnostics(&world, &line_maps, errs))?;

let standards = if cfg.accessibility.pdf_ua {
    PdfStandards::new(&[PdfStandard::Ua_1])?         // HintedStrResult; map to exit code 2
} else {
    PdfStandards::default()
};
let options = PdfOptions {
    timestamp: Some(Timestamp::new_utc(fixed_datetime)),
    creator: Smart::Custom(Some(format!("hideo {}", env!("CARGO_PKG_VERSION")))),
    standards,
    tagged: cfg.accessibility.tagged,                // default true
    ..Default::default()
};
let bytes = typst_pdf::pdf(&document, &options)
    .map_err(|errs| map_diagnostics(&world, &line_maps, errs))?;   // SourceResult<Vec<u8>>
write_atomically(&output_path, &bytes)?;
```

**Diagnostic mapping (simplified by the per-page file layout):**

- Each page is its own virtual file `pages/NNN.typ`, pulled in by `main.typ` with `#include`.
- While emitting a page, `emit.rs` records a `LineMap: Vec<u32>` where entry _i_ is the Markdown line that
  produced generated-Typst line _i_ (every `write!` of a block start pushes the current event's line).
- For each `SourceDiagnostic`: get the byte range with `typst::WorldExt::range(&world, diag.span)` and its
  file id from the span (`DiagSpan`; check the accessors in `typst_syntax::span`); look up
  `world.source(id)?.lines().byte_to_line(range.start)` (verified: `Lines::byte_to_line`) → generated line →
  `LineMap[line]` → Markdown line; the page comes from `id`.
- Diagnostics in `main.typ` or the template are **internal errors**: print them with a "this is a hideo
  bug, please report" note and write the generated sources into a temp dir.
- Warnings from `Warned::warnings` (e.g. the foreignObject warning) are mapped the same way and
  counted into the report (`warnings_as_errors` applies).
- `--keep-typst <DIR>` writes `main.typ` and `pages/*.typ` for debugging.
- PDF/UA-1 validation failures arrive as ordinary `SourceDiagnostic`s from `typst_pdf::pdf` (message prefix
  `pdf/ua-1 error:`). Those with a real span (missing alt text on an image, underline + strike combined) map
  to page and line like any other error; those with a detached span (missing document title/language) are
  hideo bugs or configuration errors. Report both as E006.

### 7.10 Accessibility (F24)

#### What Typst 0.15.1 does, verified in the sources

| Topic            | Behaviour (`typst-pdf` / `typst-library` 0.15.1)                                                                                                                                                                                                                                                                |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tagging          | `PdfOptions.tagged` defaults to `true`: a tagged PDF is written even without PDF/UA.                                                                                                                                                                                                                            |
| Table headers    | Cells in `table.header(...)` are written as `TH` with scope `Column`; rows of a table in which all rows are uniform are grouped into `THead`/`TBody`/`TFoot`. Cells outside `table.header` are `TD`. Header cells get `Headers` references so a screen reader can announce the column label for each data cell. |
| Repeated headers | Repetitions of a header on later pages are artifacts, not content.                                                                                                                                                                                                                                              |
| Scope            | Only column scope comes from `table.header`. Row headers need `pdf.header-cell` (not needed here, see F16).                                                                                                                                                                                                     |
| Table summary    | `pdf.table-summary(summary: …)` exists but needs the `a11y-extras` feature flag and is "temporary API". Not used. A figure caption is the recommended way to summarize a table.                                                                                                                                 |
| PDF/UA-1         | `PdfStandards::new(&[PdfStandard::Ua_1])` turns on validation. Violations are **errors** (the PDF is not produced): missing document title, missing document language, missing alt text on images and equations, underline/overline/strike combined, headings without text, etc.                                |
| Language         | A document language is always written; without `set text(lang: …)` it defaults to English, which would be **wrong** for non-English books. hideo therefore requires `book.lang` (D11).                                                                                                                          |
| Outline          | Headings produce the PDF outline (bookmarks) automatically.                                                                                                                                                                                                                                                     |
| Artifacts        | Page header/footer, backgrounds, shapes and paths are artifacts automatically. Decorative content can be marked with `pdf.artifact(kind: …)`; once artifact, nothing inside can be un-hidden.                                                                                                                   |
| Not guaranteed   | PDF/UA-1 validation in Typst checks structure, not meaning: a header row styled bold but emitted as plain cells passes validation and is simply `TD`. Colour contrast, reading order sense and alt-text quality are **not** validated.                                                                          |

#### Requirements

1. **Table headers.** Always `table.header(...)` for `Tag::TableHead` (7.5). Test it (below); this is the
   direct answer to "are header rows marked correctly": yes, if and only if they are emitted this way.
2. **Document metadata.** Template always sets `#set document(title: <tier title>, author: <cover author>)`
   and `#set text(lang: <book.lang>)` before any content. Title comes from `cover.title` plus the tier name.
3. **Alt text and decorative images.**
   - Hero images, cover images and placeholders are decorative by default: wrap them in
     `pdf.artifact(kind: layout)`. Rule: **decorative = artifact**; an image without `alt` that is not an
     artifact is a UA-1 error. An optional frontmatter key `hero_alt` turns a hero into real content with
     alt text (decision D9).
   - Markdown body images (D4): `![alt](path)`; non-empty `alt` → `image(…, alt: "…")`; empty alt → artifact.
   - Diagrams: alt text from, in order, the fence info string (` ```mermaid "Alt text here" `), Mermaid's
     `accTitle`/`accDescr` lines, else the generated fallback `"Diagram"` plus warning W008. Never skip: UA-1
     would fail the build.
   - The external-link arrow icon: `image(…, alt: "external link")` (spoken after the link text).
4. **Contents page.** The dotted leader (`box(width: 1fr, repeat[.])`) must be wrapped in
   `pdf.artifact(kind: layout)`; otherwise screen readers read a long run of "dot". Page numbers stay content.
5. **Headings.** One H1 per page is a style choice, not a requirement, but **skipped levels** (H1 → H3) break
   navigation: warn W007 from `check`. Headings are never used for styling only (Markdown profile guarantees it).
6. **Links.** Link text must be meaningful. Warn (not error) for link text equal to the URL or `here`/`click here`/
   `this`; do not rewrite.
7. **Language of parts.** A page with `lang` in frontmatter different from the book language wraps its body in
   `text(lang: …)`. **(verify)** that Typst writes a per-span `Lang` attribute for that; if not, document the
   limitation.
8. **Contrast.** All colours in Appendix B were computed against the WCAG 2.x formula (table in Appendix B):
   every text/background pair is ≥ 4.5:1. A unit test recomputes these from the template constants so a palette
   change cannot regress it.
9. **Reading order** follows the Typst layout order; the hero image is first on each page but is an artifact,
   so the page heading is the first thing read. Cover: title, tier line, author in that order.
10. **Configuration.** `accessibility.pdf_ua = true` by default. Turning it off is allowed (for authors who need
    to build despite a validation error) but prints a warning on every build.

#### How to verify the table header tagging (tests)

Use `lopdf` (0.47) to inspect the structure tree of the generated PDF; no external tool needed:

1. `Document::load_mem(&bytes)`; take `Catalog → StructTreeRoot` (`/K` is a child or an array of children).
2. Walk the tree recursively collecting structure elements by `/S`.
3. Assert for the `full/` fixture:
   - There is at least one `/S /Table` for each Markdown table in the source (count from the parser, not hardcoded).
   - Each `Table` has a `THead` child (or, if Typst does not emit row groups for it, a first `TR` whose cells are all `TH`).
   - Every `TH` has `/A << /O /Table /Scope /Column >>` (attribute object; exact dictionary shape **(verify)**
     against a real generated PDF in M5 and then pin it in the test).
   - Every `TD` in the body rows is **not** `TH`, and no `TH` appears in a body row (no mis-tagging).
   - For a table that breaks across pages (the `table-long` fixture) the header repetition on page 2 is
     **not** a second set of `TH` elements in the tree (it is an artifact): count of `TH` equals the number of
     header cells, regardless of the page count.
   - `Catalog /MarkInfo /Marked true`, `Catalog /Lang`, `/ViewerPreferences /DisplayDocTitle true`
     **(verify)**, `Info /Title` or XMP title present.
4. A negative control: a fixture where a table is built by hand with plain cells must produce `TD`-only, proving
   the test can fail.
5. Optional, outside the Rust test suite: **veraPDF** (`--flavour ua1`) on the `full/` fixture PDFs as a nightly
   job or manual check. It is a Java tool (also a Docker image **(verify)**) and is the standard independent
   PDF/UA-1 validator; Typst's own check is not a certification. Keep it out of the per-push pipeline until
   the baseline is clean.
6. Manual in M8: one pass with a screen reader (NVDA on Windows, free) over the cover, the contents page and one
   table-heavy page; confirm column headers are announced, dots are not read, and the arrow icon says
   "external link".

---

## 8. Testing Strategy

### 8.1 Test Levels

| Level | What                       | Tool                                                                       | Coverage Target                                                                             |
| ----- | -------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| 1     | Unit tests per requirement | `cargo test`                                                               | 100 % of F1–F23 logic                                                                       |
| 2     | Emitter snapshots          | `insta`                                                                    | Every construct in section 5                                                                |
| 3     | Escaping property          | `proptest`                                                                 | 500 cases in CI, 10 000 nightly; text must survive, not just compile                        |
| 4     | Table algorithm            | Unit tests                                                                 | Hand-computed expected widths                                                               |
| 5     | End-to-end PDF inspection  | `assert_cmd` + `lopdf`                                                     | Page count, link annotations, text extraction                                               |
| 6     | Reproducibility            | Integration test                                                           | Byte-identical across processes and `--jobs`                                                |
| 7     | Visual review              | `typst-render` PNGs + manual                                               | 25-page sample vs. Appendix B                                                               |
| 8     | Cross-tool validation      | Script (optional)                                                          | Pandoc → Typst CLI comparison on the `full/` fixture                                        |
| 9     | Accessibility              | `lopdf` struct-tree walk; veraPDF (`ua1`, nightly/manual); NVDA pass at M8 | Every table header is `TH`/column scope; PDF/UA-1 passes on `full/`; palette contrast ≥ 4.5 |

### 8.2 Detailed Test Cases by Requirement

#### F1–F4: Discovery and Frontmatter

```
test_tiers_from_config
test_empty_tier_skipped_with_warning
test_ignore_prefixes_at_depth
test_page_order_case_insensitive_natural
test_non_utf8_path_rejected
test_frontmatter_basic
test_frontmatter_quotes_stripped
test_frontmatter_duplicate_key_first_wins
test_frontmatter_indented_lines_ignored
test_frontmatter_crlf
test_frontmatter_lone_cr
test_frontmatter_missing_title_errors
test_frontmatter_missing_hero_errors
test_frontmatter_utf8_bom_stripped
test_frontmatter_colon_in_value
test_frontmatter_escaped_quotes
test_frontmatter_unclosed_errors_line_1
test_body_line_numbers_include_frontmatter_offset
```

#### F5–F8: Links

```
test_heading_id_slug_unicode (ä, ö, å)
test_heading_id_explicit
test_heading_id_explicit_invalid_errors
test_heading_id_duplicate_suffix
test_heading_id_empty_fallback_section
test_heading_id_double_space_two_hyphens
test_heading_inside_code_ignored
test_same_heading_text_on_two_pages_gets_distinct_labels
test_page_labels_unique_across_pages
test_link_external_unchanged
test_link_scheme_vs_windows_drive
test_link_percent_encoded_path
test_link_non_md_file_unchanged
test_link_same_page_anchor
test_link_same_page_no_anchor_to_first_heading
test_link_cross_page
test_link_cross_page_anchor
test_link_case_mismatch_resolves_with_warning
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
test_unsupported_tasklist_errors
test_unsupported_definition_list_errors
test_unsupported_inline_html_warns
test_html_block_errors
test_html_comment_dropped_silently
test_dollar_literal
test_heading_attributes_explicit_id_parsed
test_pathological_input_parses_fast
test_parser_panic_reported_with_page
test_hero_scaled_not_upscaled
test_hero_exif_orientation
test_hero_missing_placeholder_warning
test_hero_decompression_bomb_rejected
test_cover_full_bleed
test_contents_two_columns
test_contents_groups_by_folder
test_contents_omits_pages_without_headings
test_contents_page_numbers_match_actual_pages (lopdf: extract text, compare)
test_page_number_on_all_but_cover
test_diagram_external_renders
test_diagram_foreignobject_rejected
test_diagram_failure_names_page_and_line
test_diagram_timeout_kills_child
test_diagram_cache_hit_skips_backend
test_table_narrow_one_size
test_table_wide_reduced_font
test_table_long_word_breaks
test_table_header_repeats (lopdf: header text on multiple pages)
test_table_header_cells_tagged_th_column_scope (lopdf struct tree, see 7.10)
test_table_body_cells_tagged_td_not_th
test_table_header_not_duplicated_in_struct_tree_across_pages
test_table_negative_control_plain_row_is_td
test_table_empty_header_cell_warns
test_pdf_has_marked_lang_and_title
test_pdf_ua1_build_passes_on_full_fixture
test_pdf_ua1_missing_alt_text_is_error_with_page_and_line
test_decorative_hero_is_artifact
test_contents_leader_is_artifact (no run of dots in extracted text)
test_external_link_arrow_has_alt
test_diagram_alt_from_fence_info_then_acctitle_then_warning
test_heading_skipped_level_warns
test_palette_contrast_at_least_4_5
test_lead_in_label
test_lead_in_not_when_split_strong
test_external_link_marker
```

#### F19–F23: Output and Behavior

```
test_reproducible_two_builds_identical (separate processes, --jobs 1 vs 8)
test_timestamp_from_source_date_epoch
test_invalid_source_date_epoch_exits_2
test_error_names_page_and_line
test_typst_error_maps_to_markdown_line
test_warnings_collected_and_counted
test_warnings_as_errors
test_no_partial_pdf_on_error
test_check_mode_no_pdf_written
test_check_mode_json_format
test_exit_codes
test_config_unknown_key_errors
test_config_relative_paths_from_config_file
test_config_defaults
test_config_missing_book_lang_exits_2
test_config_invalid_book_lang_exits_2
test_pdf_ua_off_prints_warning
test_hero_alt_makes_hero_content_with_alt
test_hero_without_alt_is_artifact
```

### 8.3 Property Tests

| Property        | Input                                            | Assertion                                                                      |
| --------------- | ------------------------------------------------ | ------------------------------------------------------------------------------ |
| Escaping        | Random Unicode string                            | Typst compiles, no warnings, extracted text equals the input modulo shorthands |
| Table widths    | Random column counts (1–10), random text lengths | Fractions sum to 1.0, each ≥ 0                                                 |
| Slug uniqueness | Random heading lists                             | All IDs unique                                                                 |
| Link rewriting  | Random path + anchor combinations                | Output is unchanged or a valid Typst link (compiles)                           |
| Line index      | Random text with LF/CRLF-normalized endings      | `byte → line` agrees with a naive counter                                      |

### 8.4 End-to-End Fixtures

1. **`minimal/`**: 2 pages, 1 tier, no images, no diagrams.
2. **`full/`**: 20 pages, 3 tiers, all constructs, 5 diagrams (pre-rendered SVGs checked in so CI does not need
   Node/Chromium), 10 images.
3. **`edge/`**: Unicode headings, long table strings, missing heroes, unresolved links, pathological Markdown.
4. **`repro/`**: same as `minimal/`, built twice.
5. **`windows-paths/`**: links using `%20`, mixed case and nested folders (exercises F7 on all OSes).

Keep fixture images tiny (a few KB, generated procedurally) so the repo stays small; the 260 MB
performance test uses a **generated** corpus (`cargo xtask gen-corpus` or a shell script) and runs
manually or nightly, not on every push.

### 8.5 Visual Review Protocol (M6, M8)

1. Build `full/`.
2. Render pages 1 (cover), 2 (contents), 3 (first content page) and a table-heavy page to PNG with
   `typst_render::render` (no `pdftoppm` needed; run it in a test with `HIDEO_UPDATE_VISUALS=1` to refresh).
3. Compare side-by-side with Appendix B. Check font sizes, colours, margins, spacing, table lines, quote
   bar, code-block radius, hero bleed.
4. Commit the approved PNGs and compare future runs with `image-compare` (≤ 0.1 % differing pixels).
5. Record deviations as issues; fix before M8.

### 8.6 CI Pipeline (Semaphore)

Semaphore facts used here (docs.semaphore.io, checked 2026-11-17):

- The docs moved from `docs.semaphoreci.com` to **`docs.semaphore.io`**; update links in the README.
- Linux x86_64: `f1-standard-2` (2 vCPU/8 GB) and `f1-standard-4` with `os_image: ubuntu2404`. The existing
  config uses `e1-standard-2` + `ubuntu2004`; E1/E2 are "legacy" and the 20.04 image is not in the current
  list. Use F1 + `ubuntu2404`.
- Linux ARM: `r1-standard-2` (`ubuntu2404`, ARM version only; no Docker environments).
- macOS (Apple Silicon): `a2-standard-4` with the `macos-xcode26` (or Xcode27) image. **(verify)** the exact
  `os_image` string in the workflow editor.
- **Windows: not offered on Semaphore Cloud.** Windows is covered by **AppVeyor** (decided; see "Windows CI"
  below). Semaphore does not wait for it; the two systems report status to GitHub independently.
- Rust: the Ubuntu 24.04 image page lists "Rust 1.98.1", but the Rust language guide says Rust is not
  preinstalled, and `sem-version` does **not** support Rust. So the prologue installs rustup if it is
  missing and lets `rust-toolchain.toml` pick the toolchain. First CI run: print `rustc -V` and decide
  whether the install step can be deleted **(verify)**.
- Toolbox: `cache store/restore <keys> [path]` (comma-separated keys; first match wins), `checksum <file>`,
  `artifact push|pull <job|workflow|project> <path>`.

`.semaphore/semaphore.yml`:

```yaml
# https://docs.semaphore.io/reference/pipeline-yaml
version: v1.0
name: hideo
agent:
  machine:
    type: f1-standard-2
    os_image: ubuntu2404

auto_cancel:
  running:
    when: "branch != 'main'"

fail_fast:
  stop:
    when: "branch != 'main'"

global_job_config:
  env_vars:
    - name: CARGO_TERM_COLOR
      value: always
    - name: CARGO_INCREMENTAL
      value: "0"
  prologue:
    commands:
      - checkout
      - 'command -v rustup >/dev/null || curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal'
      - export PATH="$HOME/.cargo/bin:$PATH"
      - rustup show # installs the toolchain from rust-toolchain.toml
      - rustc -V && cargo -V
      - cache restore cargo-registry-$(checksum Cargo.lock),cargo-registry
      - cargo fetch --locked
  epilogue:
    on_pass:
      commands:
        - cache has_key cargo-registry-$(checksum Cargo.lock) || cache store cargo-registry-$(checksum Cargo.lock) ~/.cargo/registry

blocks:
  - name: Lint
    run:
      when: "change_in(['/src/', '/tests/', '/assets/', '/Cargo.toml', '/Cargo.lock', '/rust-toolchain.toml', '/.semaphore/'])"
    task:
      jobs:
        - name: fmt and clippy
          commands:
            - cargo fmt --all --check
            - cargo clippy --all-targets --locked -- -D warnings

  - name: Test (Linux x86_64)
    run:
      when: "change_in(['/src/', '/tests/', '/assets/', '/Cargo.toml', '/Cargo.lock', '/rust-toolchain.toml', '/.semaphore/'])"
    task:
      prologue:
        commands:
          - cache restore target-linux-$(checksum Cargo.lock),target-linux
      jobs:
        - name: cargo test
          commands:
            - cargo test --locked
      epilogue:
        on_pass:
          commands:
            - cache has_key target-linux-$(checksum Cargo.lock) || cache store target-linux-$(checksum Cargo.lock) target

  - name: MSRV (Rust 1.92)
    run:
      when: "change_in(['/src/', '/Cargo.toml', '/Cargo.lock', '/.semaphore/'])"
    task:
      jobs:
        - name: cargo check on MSRV
          commands:
            - rustup toolchain install 1.92 --profile minimal
            - cargo +1.92 check --locked

  - name: Test (Linux ARM)
    run:
      when: "change_in(['/src/', '/tests/', '/assets/', '/Cargo.toml', '/Cargo.lock', '/.semaphore/'])"
    task:
      agent:
        machine:
          type: r1-standard-2
          os_image: ubuntu2404
      jobs:
        - name: cargo test
          commands:
            - cargo test --locked

  - name: Test (macOS)
    run:
      when: "change_in(['/src/', '/tests/', '/assets/', '/Cargo.toml', '/Cargo.lock', '/.semaphore/'])"
    task:
      agent:
        machine:
          type: a2-standard-4
          os_image: macos-xcode26 # verify exact name in the workflow editor
      jobs:
        - name: cargo test
          commands:
            - cargo test --locked

  - name: Release build and end-to-end
    run:
      when: "change_in(['/src/', '/tests/', '/assets/', '/Cargo.toml', '/Cargo.lock', '/.semaphore/'])"
    task:
      jobs:
        - name: build, build fixture twice, compare
          commands:
            - cargo build --release --locked
            - export SOURCE_DATE_EPOCH=1700000000
            - ./target/release/hideo build --input tests/fixtures/full --output out1 --jobs 1
            - ./target/release/hideo build --input tests/fixtures/full --output out2 --jobs 4
            - for f in out1/*.pdf; do cmp "$f" "out2/$(basename "$f")"; done
            - artifact push workflow target/release/hideo
            - artifact push workflow out1 --destination fixture-pdfs

promotions:
  - name: Release
    pipeline_file: release.yml
    auto_promote:
      when: "tag =~ '^v[0-9]+\\.[0-9]+\\.[0-9]+$' AND result = 'passed'"
```

Notes:

- `[skip ci]` in a commit message is honoured by Semaphore natively; the `change_in(...)` guards give the
  same effect as the old AppVeyor `skip_commits` rule (docs-only commits touching `*.md`/`LICENSE*` do not
  start jobs). **(verify)** `change_in` accepts a list in this form in the workflow editor.
- `target/` cache can reach several GB with Typst. Measure the first run; if restore+store costs more than
  the saved compile time, keep only the registry cache.
- The old `semaphore.yml` is a Node.js pipeline (`npm ci`, `ubuntu2004`) and fails today. Replace it when
  the Rust skeleton lands (M1), not before, or the pipeline fails on a repo with no `Cargo.toml`.
- Keep the README badge; Semaphore org URLs now live under `*.semaphore.io` **(verify)** whether the
  `paazmaya.semaphoreci.com` badge URL still redirects.

**Windows CI: AppVeyor (decided).** The repo used AppVeyor until commit `a4405d4` removed the Node.js
`appveyor.yml`; the new file keeps its conventions (`Visual Studio 2022` image, `branches: only: main`,
`skip_commits` for docs-only commits, `clone_depth: 1`, `core.longpaths`, `build: off`) and replaces the Node
steps with Rust. Windows is the platform where path, CRLF and `\\?\` behaviour matter, so the same
`cargo test` plus the `windows-paths/` fixture and the reproducibility check run there.

`appveyor.yml`:

```yaml
# https://www.appveyor.com/docs/appveyor-yml/
image:
  - Visual Studio 2022

branches:
  only:
    - main

skip_branch_with_pr: true

skip_commits:
  files:
    - "*.md"
    - "LICENSE*"

version: "{build}-{branch}"

environment:
  CARGO_TERM_COLOR: always
  CARGO_INCREMENTAL: 0
  RUST_BACKTRACE: 1

init:
  - git config --global core.longpaths true

clone_depth: 1

cache:
  - '%USERPROFILE%\.cargo\registry -> Cargo.lock'
  - target -> Cargo.lock, rust-toolchain.toml, appveyor.yml

install:
  - ps: |
      if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        Invoke-WebRequest https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
        .\rustup-init.exe -y --default-host x86_64-pc-windows-msvc --profile minimal
        Remove-Item rustup-init.exe
      }
  - set PATH=%USERPROFILE%\.cargo\bin;%PATH%
  - rustup show
  - rustc -V
  - cargo -V

build: off

test_script:
  - cargo fmt --all --check
  - cargo clippy --all-targets --locked -- -D warnings
  - cargo test --locked
  - cargo build --release --locked
  - set SOURCE_DATE_EPOCH=1700000000
  - target\release\hideo.exe build --input tests\fixtures\full --output out1 --jobs 1
  - target\release\hideo.exe build --input tests\fixtures\full --output out2 --jobs 4
  - ps: Get-ChildItem out1\*.pdf | ForEach-Object { if ((Get-FileHash $_.FullName).Hash -ne (Get-FileHash "out2\$($_.Name)").Hash) { throw "not reproducible: $($_.Name)" } }

after_test:
  - ps: |
      if ($env:APPVEYOR_REPO_TAG -eq "true") {
        $v = $env:APPVEYOR_REPO_TAG_NAME
        $dir = "hideo-$v-x86_64-pc-windows-msvc"
        New-Item -ItemType Directory $dir | Out-Null
        Copy-Item target\release\hideo.exe, LICENSE, README.md $dir
        Compress-Archive $dir "$dir.zip"
        (Get-FileHash "$dir.zip" -Algorithm SHA256).Hash.ToLower() + "  $dir.zip" | Set-Content "$dir.zip.sha256"
        Push-AppveyorArtifact "$dir.zip"
        Push-AppveyorArtifact "$dir.zip.sha256"
      }

artifacts:
  - path: out1\*.pdf
    name: fixture-pdfs

deploy:
  - provider: GitHub
    auth_token:
      secure: <encrypted GitHub token>       # Account → Encrypt YAML; never commit the plain token
    artifact: /hideo-.*\.zip.*/
    tag: $(APPVEYOR_REPO_TAG_NAME)
    draft: true                             # review, then publish by hand; the Semaphore jobs add their archives
    on:
      APPVEYOR_REPO_TAG: true
```

Notes:

- `skip_branch_with_pr: true` and `branches: only: main` mean pull-request branches are built **once**, as PR
  builds, not twice. Keep both from the old file.
- `build: off` keeps AppVeyor's MSBuild stage out of the way; everything is a script.
- `branches: only: main` also means tag builds need either `main`-only tags or a `tags` rule: AppVeyor builds
  tags by default but the branch filter applies to the tag's commit **(verify)**; if the release tag build does
  not start, add `- /^v\d+\.\d+\.\d+$/` under `only`.
- The `deploy` section follows AppVeyor's GitHub provider documentation (provider name is case sensitive;
  `artifact` matches by file name or regex; `on: APPVEYOR_REPO_TAG: true` limits to tag pushes). Creating the
  release as a **draft** lets the Semaphore Linux/macOS jobs attach their archives to the same release (8.7)
  before it is published.
- Rust on the AppVeyor image: whether rustup is preinstalled on `Visual Studio 2022` could not be confirmed
  (the AppVeyor Rust docs page returned 404; community templates install rustup with `rustup-init.exe`). The
  `install` step installs it only if `rustup` is missing. **(verify)** on the first run and delete the
  install if it is already there.
- AppVeyor's cache is size-limited per project **(verify)**; if the `target` cache exceeds it or is slower
  than rebuilding, keep only the registry cache.
- Compile errors with MSVC build tools are not expected (Typst crates are pure Rust); no extra Visual Studio
  components are needed. If a dependency adds a C build script, the VS 2022 image already has the MSVC toolchain.
- On Windows, the file `C:\…` path forms and `\\?\` prefixes are the likely failure area; the test names in 8.2
  (`test_link_scheme_vs_windows_drive`, `test_non_utf8_path_rejected`, the `windows-paths/` fixture) are the
  regression net.

### 8.7 Release Pipeline (`.semaphore/release.yml`, M9)

- Triggered by a version tag via the promotion above.
- Jobs build `--release --locked` per target: `x86_64-unknown-linux-gnu` (f1), `aarch64-unknown-linux-gnu`
  (r1), `aarch64-apple-darwin` (a2) and, from the same Mac, `x86_64-apple-darwin`
  (`rustup target add x86_64-apple-darwin`). Windows `x86_64-pc-windows-msvc` is built and uploaded by the
  AppVeyor `after_test`/`deploy` steps above (a draft GitHub release named after the tag).
- Archive as `hideo-<version>-<target>.tar.gz` (`.zip` on Windows) with `LICENSE` and `README.md`; write a
  `SHA256SUMS` file.
- Upload with `gh release upload` using a token stored as a Semaphore secret (`sem create secret`), referenced
  in the job's `secrets:`. Never print the token. Create the release as a **draft** only if it does not exist
  yet (`gh release view "$TAG" || gh release create "$TAG" --draft`), because AppVeyor's GitHub provider also
  creates or updates the draft for the same tag. Publish by hand once all five archives and the checksums are
  attached.
- Publish to crates.io manually (`cargo publish`) after the first release; typst crates are exact-pinned,
  which is allowed for a binary crate.
- `cargo-dist` can still be used **locally** for installer generation, but its CI generation is GitHub-only,
  so it is not part of the pipeline.

---

## 9. Milestones

| M   | Milestone         | Requirements                | Done When                                                                                                                                               | Days   |
| --- | ----------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 1   | Skeleton + Config | F1–F4, F21, F22             | Cargo project, **Semaphore and AppVeyor green on the skeleton**, CLI parses, config loads, discovery + frontmatter work, `list` and `check` on fixtures | 2      |
| 2   | Link Model        | F5–F8                       | All link tests pass, report format correct                                                                                                              | 2      |
| 3   | Images            | F10–F12                     | Heroes scaled/cached/placeholdered, cover generated, resize benchmark recorded                                                                          | 2      |
| 4   | Emitter Core      | F9, F17, F18                | Headings, paragraphs, emphasis, links, lists, quotes, code, rules, escaping; snapshot + property tests pass                                             | 5      |
| 5   | Tables            | F16, F24                    | Table fixtures pass, font reduction works, long words break, **header rows verified as `TH`/`THead` in the struct tree**                                | 4      |
| 6   | Typst Integration | F10, F12–F14, F19, F20, F24 | `World` on `typst-kit`, template, cover, contents with page numbers, PDF export with PDF/UA-1 validation passing, reproducible, error mapping           | 5      |
| 7   | Diagrams          | F15                         | Trait, external backend, native evaluation, **M7 gate decision**                                                                                        | 4      |
| 8   | Hardening         | All                         | Full content builds, performance goal met, visual review pass, veraPDF baseline and one screen-reader pass (7.10)                                       | 4      |
| 9   | Packaging         | F21, F23                    | `--help` polish, README, release pipeline (8.7)                                                                                                         | 2      |
|     | **Total**         |                             |                                                                                                                                                         | **30** |

**Order notes:**

- M4 and M5 must complete before M6, so emitter bugs are caught before Typst compilation.
- A **thin vertical slice of M6** (World + compile a hard-coded one-page document → PDF → lopdf reads it) is
  worth doing on day 1 of M1. It de-risks the reworked `typst-kit` API and the CI toolchain at the same time,
  and it is the single biggest uncertainty in the plan.
- M7 can run in parallel with M5–M6 (diagrams are independent of table/image work).
- M3's image logic can be written before M6; hero _placement_ needs M6 to verify.
- M6 is still the riskiest milestone but less so than before: the 0.15 `typst-kit` API has been read and the
  `World` skeleton in 7.8 is derived from the real sources. Remaining unknowns are template-level (pull hero
  into margins, non-breaking table rows, contents leaders).

---

## 10. Risks

| Risk                                                                                                                | Likelihood                           | Effect                                                    | Mitigation                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Typst API changes                                                                                                   | Medium                               | Rework in `world.rs`/`compile.rs`                         | Pin `=0.15.1`; confine Typst to those two files; group-bump in Renovate; upgrade deliberately                                                                                            |
| Template relies on Typst behaviour that differs from expectation (hero bleed, non-splitting rows, contents leaders) | Medium                               | Visual regressions                                        | Day-1 vertical slice; `typst-render` snapshots at M6                                                                                                                                     |
| pulldown-cmark slow input or panic                                                                                  | Low–Medium                           | Hang or crash on a page                                   | 0.13.6+, `catch_unwind` (do not set `panic = "abort"`), pathological fixture, no known control-character bug                                                                             |
| No native Mermaid renderer passes the gate                                                                          | Medium                               | Not fully standalone                                      | External backend stays; per-diagram-type fallback; document limitation                                                                                                                   |
| Mermaid SVG uses `<foreignObject>`                                                                                  | **High** with `mmdc` defaults        | Missing labels in PDF                                     | `htmlLabels: false` in theme; hard post-condition check; warning from Typst is surfaced                                                                                                  |
| SVG fonts not embedded                                                                                              | Medium                               | Wrong face, clipped labels                                | `fontFamily` in theme; post-condition check; M7 contact sheet at final scale                                                                                                             |
| Hand-written emitter edge case                                                                                      | **High**                             | Broken pages                                              | Fixtures, snapshots, property test that checks text survives, strict errors; escape-hatch design B                                                                                       |
| Table widths wrong                                                                                                  | Medium                               | Page breaks shift                                         | Literal algorithm, hand-computed fixtures, visual review                                                                                                                                 |
| Font coverage (non-Latin characters)                                                                                | Low–Medium                           | Missing-glyph boxes                                       | Coverage warning in `check`; document supported scripts                                                                                                                                  |
| Windows path/CRLF issues                                                                                            | Medium                               | Failures on Windows                                       | Lexical path normalization, `camino`, normalized CRLF, `windows-paths/` fixture; real Windows runner (AppVeyor) on every push                                                            |
| Two CI systems (Semaphore + AppVeyor) drift apart                                                                   | Medium                               | Different toolchain or flags per platform                 | Same commands in both files (fmt, clippy, test, release build, reproducibility check); `rust-toolchain.toml` is the single source of the Rust version; Renovate does not touch CI images |
| AppVeyor image lacks or changes Rust/VS tooling                                                                     | Low–Medium                           | Windows job red for non-code reasons                      | `install` step installs rustup when missing; first-run check (8.6)                                                                                                                       |
| Header row emitted as plain cells instead of `table.header`                                                         | Medium (easy mistake in the emitter) | Table announced without column headers; PDF still "valid" | Emitter has one table function; struct-tree tests incl. negative control (7.10)                                                                                                          |
| PDF/UA-1 validation fails builds on content problems (missing alt, empty title)                                     | Medium                               | Authors blocked                                           | Errors name page and line (E006); `accessibility.pdf_ua = false` escape hatch; `check` warns first (W006–W008)                                                                           |
| Typst PDF/UA-1 support is structural only                                                                           | Medium                               | False sense of compliance                                 | Say so in README; veraPDF baseline; screen-reader pass at M8; contrast test                                                                                                              |
| Decorative images wrongly exposed or real images hidden                                                             | Low                                  | Noise or lost information for screen-reader users         | Rule "decorative = artifact"; optional `hero_alt`; tests in 8.2                                                                                                                          |
| Semaphore Rust toolchain availability unclear (docs conflict)                                                       | Medium                               | First CI runs fail                                        | Prologue installs rustup; print `rustc -V`                                                                                                                                               |
| Large inputs memory/time                                                                                            | Low                                  | Out-of-memory, slow builds                                | Image cache, decode limits, in-memory images, measure at M8                                                                                                                              |
| `FileId` table limit (65,535)                                                                                       | Very low                             | Panic on huge books                                       | Intern each file once; assert in `check`                                                                                                                                                 |
| Scope creep                                                                                                         | Medium                               | Schedule slip                                             | Unsupported-construct error keeps scope fixed                                                                                                                                            |

---

## 11. Decisions Needed Before Starting

| ID  | Decision                                                                                                                                                                                                | Recommendation                                                  |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| D1  | ~~Windows CI~~ **Decided: AppVeyor** (8.6).                                                                                                                                                             | done                                                            |
| D9  | ~~Hero image alt text~~ **Decided:** heroes are decorative (artifacts) by default; an optional `hero_alt` frontmatter key makes a hero real content with alt text.                                      | done                                                            |
| D10 | ~~PDF/UA-1 validation default~~ **Decided:** on by default (builds fail on violations); `accessibility.pdf_ua = false` is the escape hatch and prints a warning on every build.                         | done                                                            |
| D11 | ~~Book language~~ **Decided:** `book.lang` is required in the config, no default, so the document language is never silently wrong.                                                                     | done                                                            |
| D2  | **Diagrams.** Accept "standalone except diagrams" (or per-type fallback) if no native renderer passes the gate, or hold the release?                                                                    | Accept per-type fallback; do not hold the release               |
| D3  | **Visual tolerance.** Is following Appendix B plus the level-5 checks enough, or must pages match reference PDFs line by line?                                                                          | Appendix B + committed PNG snapshots with ≤ 0.1 % tolerance     |
| D4  | ~~Body images~~ **Implemented as recommended:** supported, scaled to the text width and shrunk to at most 85 % of the text height, same cache. Empty alt text makes the image decorative (an artifact). | done                                                            |
| D5  | ~~Links that leave the tier~~ **Implemented as recommended:** plain text, reported as _outside this PDF_ (W001).                                                                                        | done                                                            |
| D6  | ~~Page numbering~~ **Implemented as recommended:** the cover is page 1 (no number), the contents page shows 2.                                                                                          | done                                                            |
| D7  | **Cover text.** Title, author and the line template are configuration (Appendix A); confirm the wording.                                                                                                | n/a (content decision)                                          |
| D12 | ~~Pages without a heading~~ **Decided during implementation:** the frontmatter `title` becomes an H1 when the body has no heading, so every page has a heading and a contents entry.                    | done                                                            |
| D13 | ~~Diagram and image size~~ **Decided during implementation:** images fit the text width, diagrams are never scaled above their natural size, both at most 85 % of the text height (note N4).            | done                                                            |
| D8  | **Release targets.** Which OS/arch binaries are published?                                                                                                                                              | linux x86_64 + aarch64, macOS aarch64 (+x86_64), Windows x86_64 |

(The previous revision numbered these 2, 3, 5; items 1 and 4 were missing from the text and have been
dropped or re-added as D4/D5. Reinstate them if they were meaningful.)

---

## Appendix A: Configuration Example

```toml
# hideo.toml
tiers = ["required", "additional", "bonus"] # folder names; also the PDF names
ignore_prefixes = ["_", "."]

[page]
width_mm = 210
margin_x_mm = 20
margin_y_mm = 18
hero_height_mm = 118.125 # placeholder height, 16:9

[images]
dpi = 200 # heroes are scaled to the page width at this resolution
jpeg_quality = 70
contents_hero = "images/contents.png"
cover_pattern = "images/cover_front_{tier}.png" # grey-violet placeholder when missing

[cover]
title = "Book title"
author = "Author name"
tier_line = "{Tier} course material"
panel_width_mm = 170
panel_offset_mm = 20

[contents]
columns = 2

[diagrams]
backend = "external" # "native", "external" or "off"
command = "mmdc" # or "mmdr"; used by the external backend
args = ["-i", "{input}", "-o", "{output}", "-c", "{theme}", "-e", "svg"]
timeout_s = 30
theme = "assets/mermaid-theme.json"

[build]
cache_dir = ".hideo-cache" # relative to the output folder

[diagnostics]
warnings_as_errors = false

[book]
lang = "fi" # REQUIRED document language (BCP 47); see 7.10 and D11

[accessibility]
tagged = true # write a tagged PDF
pdf_ua = true # enforce PDF/UA-1 validation
```

---

## Appendix B: Visual Specification

All sizes and colours below are requirements and live in the template (`assets/template.typ`), with the
page geometry filled in from the configuration.

| Element        | Specification                                                                                                                                                                                                                                                             |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Page           | A4 portrait, margins from the configuration; footer: page number centred, 8 pt, `#6E6682`                                                                                                                                                                                 |
| Body text      | Libertinus Serif 10.5 pt, `#2A2438`, not justified, leading 0.75 em, paragraph spacing 0.65 em, language from frontmatter                                                                                                                                                 |
| Heading 1      | 19 pt bold, `#24134D`, 1.6 em above and 1.5 em below, kept with the next block                                                                                                                                                                                            |
| Heading 2      | 15 pt bold, `#3D2A7A`, 2.4 em above and 1.4 em below, kept with the next block                                                                                                                                                                                            |
| Heading 3      | 12 pt bold, `#52389A`, 1.8 em above and 1.1 em below, kept with the next block                                                                                                                                                                                            |
| Headings       | not numbered                                                                                                                                                                                                                                                              |
| Lists          | 1.3 em of space after a list                                                                                                                                                                                                                                              |
| Code block     | full width, fill `#F3F0F8`, 9 pt inset, 4 pt radius, DejaVu Sans Mono 8 pt                                                                                                                                                                                                |
| Links          | `#4A2F8F`; links that leave the PDF are followed by a small arrow icon (an inline SVG, 0.6 em high, stroke `#4A2F8F`)                                                                                                                                                     |
| Block quote    | full width, fill `#F0EBF8`, inset 8 pt vertical and 10 pt horizontal, a 2.5 pt left bar `#5B3FA0`, 3 pt radius on the right, 1 em above and below, text 10 pt, paragraph spacing 0.6 em                                                                                   |
| Lead-in label  | bold, `#5B3FA0`                                                                                                                                                                                                                                                           |
| Thematic break | centred line, 30 % of the width, 0.6 pt, `#D3CCE4`, 1.4 em above and below                                                                                                                                                                                                |
| Tables         | cell inset 4 pt by 4 pt; only horizontal lines, 0.4 pt `#D3CCE4`; header row fill `#E9E3F6` and bold; every second body row fill `#F5F3F9`; may break across pages; rows are not split when they fit; hyphenation on; 1.8 em above and below; header repeats on each page |
| Page head      | hero full width pulled into the margins, 5 mm space; subtitle 12 pt `#6E6682` with 8 pt below; author line 9 pt `#6E6682` followed by 5 mm; placeholder fill `#E7E3EF`                                                                                                    |
| Contents       | title "Contents" 19 pt bold `#24134D`; two columns with 6 mm gutter; text 7.5 pt, leading 0.4 em; group label 8.5 pt bold `#3D2A7A`; entry: title, dotted leader, page number, text `#2A2438`, 0.55 em above                                                              |
| Cover          | full-bleed, panel `#2E1A5C`, white text; title 24 pt bold, tier line 16 pt, author 12 pt; panel inset 16 pt, radius 4 pt; placeholder `rgb(231, 227, 239)`                                                                                                                |

Accessibility notes for the table above (F24, 7.10): the table header row is `table.header(...)`, not a styled
first row; the contents leader and the hero/cover images are artifacts; the external-link arrow has alt text
"external link".

**Contrast check** (WCAG 2.x relative luminance, computed 2026-11-17; AA needs ≥ 4.5:1 for normal text):

| Text on background                                              | Ratio                                                                 |
| --------------------------------------------------------------- | --------------------------------------------------------------------- |
| Body `#2A2438` on white                                         | 14.92                                                                 |
| H1 `#24134D` / H2 `#3D2A7A` / H3 `#52389A` on white             | 16.47 / 11.60 / 8.78                                                  |
| Footer and subtitle `#6E6682` on white                          | **5.40** (the lowest text pair in the spec; 8 pt footer still passes) |
| Links `#4A2F8F` on white / on quote fill `#F0EBF8`              | 10.03 / 8.57                                                          |
| Lead-in `#5B3FA0` on white / on quote fill                      | 7.88 / 6.74                                                           |
| Quote text `#2A2438` on `#F0EBF8`                               | 12.75                                                                 |
| Code text `#2A2438` on `#F3F0F8`                                | 13.23                                                                 |
| Table text `#2A2438` on header fill `#E9E3F6` / zebra `#F5F3F9` | 11.92 / 13.55                                                         |
| Cover white on `#2E1A5C`                                        | 14.80                                                                 |

Caveat: `#6E6682` on the placeholder fill `#E7E3EF` is only 4.28:1. The spec has no text on the placeholder; if
one is ever added, use `#2A2438`. The table-rule colour `#D3CCE4` is decorative (not text) and exempt.
Contrast is not checked by Typst's PDF/UA validation; `test_palette_contrast_at_least_4_5` covers it.

Font note: only Libertinus Serif and DejaVu Sans Mono are used, both embedded; nothing in the spec requires a
sans-serif face. Bold/italic variants of both are embedded (Libertinus Regular/Italic/Semibold/Bold/BoldItalic/SemiboldItalic; DejaVu Sans Mono Regular/Bold/Oblique/BoldOblique).

---

## Appendix C: Default Diagram Theme

Passed to the diagram renderer as JSON in Mermaid's configuration format (the renderer must support the settings
it is given; settings it does not know are ignored). Changes from the previous revision: added
`fontFamily` (SVG text is drawn with Typst's embedded fonts) and `flowchart.htmlLabels: false` (avoids
`<foreignObject>`, which Typst renders incorrectly).

```json
{
  "theme": "base",
  "look": "neo",
  "themeVariables": {
    "fontFamily": "Libertinus Serif",
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
    "htmlLabels": false,
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

`"look": "neo"` is a Mermaid 11.x/12 feature (introduced in the 11.x line per third-party sources; the official
docs describe it with the `redux` theme family) and is ignored by older mermaid-cli and by the native
renderers. **(verify)** the installed `mmdc` version; also verify that `"radius"` and the `cScale*` keys are
honoured by the chosen renderer, since `mmdr`'s config loader only reads the keys it has fields for.

---

## Appendix D: Corrections to the Previous Revision

| Previous claim                                                                                                                                                                                           | Finding                                                                                                                                                                                          |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `typst_pdf::pdf(&doc, &options)` returns the bytes                                                                                                                                                       | Returns `SourceResult<Vec<u8>>`.                                                                                                                                                                 |
| `PdfOptions.timestamp` takes a `chrono` value                                                                                                                                                            | It is `Option<typst_pdf::Timestamp>`; build it with `Timestamp::new_utc(Datetime)`.                                                                                                              |
| `typst::compile(&world)` and `result.is_ok()` in the proptest                                                                                                                                            | It is `compile::<PagedDocument>(&world)` returning `Warned<SourceResult<_>>`; `PagedDocument` is in `typst_layout`.                                                                              |
| `World::today(&self, offset: Option<i64>)`                                                                                                                                                               | Takes `Option<typst::foundations::Duration>`.                                                                                                                                                    |
| `FileId::new(None, VirtualPath::new(...))`                                                                                                                                                               | Now `FileId::new(RootedPath::new(VirtualRoot::Project, VirtualPath::new(..)?))`.                                                                                                                 |
| `typst_kit::fonts::FontSearcher` with `include_system_fonts(false)` / `include_embedded_fonts(true)`                                                                                                     | Does not exist in 0.15; use `FontStore` + `fonts::embedded()`.                                                                                                                                   |
| Depend on `typst-assets` directly                                                                                                                                                                        | Pulled in transitively by `typst-kit/embedded-fonts`.                                                                                                                                            |
| pulldown-cmark 0.13.4 panics in `OffsetIter` on NUL/VT/FF in link reference definitions                                                                                                                  | No source found. Release notes: 0.13.4 fixed a `TightParagraph` panic; 0.13.5 fixed quadratic-time parsing and a definition-list offset panic; 0.13.6 fixed a wikilink DoS. Use 0.13.6.          |
| pulldown-cmark "will parse some [footnotes etc.] by default"                                                                                                                                             | Extensions are opt-in. They must be enabled to be detected. Also `ENABLE_HEADING_ATTRIBUTES` is required for the `{#id}` syntax and the previous option list omitted it.                         |
| Manually replace `...` with `…` while letting Typst handle punctuation                                                                                                                                   | Typst already converts `...`; the replacement is redundant.                                                                                                                                      |
| `toml = "0.8"`                                                                                                                                                                                           | Current is 1.2.                                                                                                                                                                                  |
| `lopdf = "0.44"`                                                                                                                                                                                         | Current is 0.47; `get_page_annotations` exists.                                                                                                                                                  |
| `cargo-dist` 0.30.x for releases                                                                                                                                                                         | Current is 0.34 (binary `dist`), but its only CI backend is GitHub; unsuitable with Semaphore.                                                                                                   |
| "Use Semaphore CI" with a GitHub Actions YAML and a macOS matrix                                                                                                                                         | Replaced by a Semaphore pipeline. Semaphore Cloud has no Windows, so Windows runs on AppVeyor (decided in revision 3, 8.6).                                                                      |
| No accessibility requirements (revision 2)                                                                                                                                                               | Added F24 and 7.10. The only table-header rule was "header repeats on each page", which is a layout property; it says nothing about tagging. Header rows must be `table.header(...)` to be `TH`. |
| `command = "mermaid-renderer"`                                                                                                                                                                           | Not a real binary. The mermaid-cli binary is `mmdc`; `mmdr` is the native CLI. Both accept `-i -o -c -e`.                                                                                        |
| Appendix C theme has no font and default HTML labels                                                                                                                                                     | Typst resolves SVG text through its font book and warns on `<foreignObject>`; fixed in the theme.                                                                                                |
| Typst 0.15.1 changelog details (math alignment, `op` misalignment, SVG pretty printing); "`box`/`block` purpose aligned with paged export"; "the typst-py release added a reproducible timestamp option" | Not verifiable from the sources; removed as justification.                                                                                                                                       |
| Contents/footers have no description of how page numbers are obtained                                                                                                                                    | Added (F13, `context` + `locate`).                                                                                                                                                               |
| One concatenated `main.typ` with a hand-built source map                                                                                                                                                 | Replaced by per-page virtual files + a per-page line map (7.9).                                                                                                                                  |
| Cache dir relative to the CWD (7.4) but "in the output folder" (7.6)                                                                                                                                     | Output folder.                                                                                                                                                                                   |
| Decision list numbered 2, 3, 5                                                                                                                                                                           | Renumbered D1–D8.                                                                                                                                                                                |
| Requirements F1–F23 "as in the original plan"                                                                                                                                                            | The list is absent from this file; reconstructed in 4.0. Confirm.                                                                                                                                |

---

## Appendix E: Repository Bootstrap Checklist (do in M1)

The working tree is what is left after the Node.js tool was removed (`a4405d4`).

- [x] `cargo init --name hideo` (binary crate); add the `Cargo.toml` pieces from 3.4, 6 and 7.1; commit `Cargo.lock`.
- [x] `rust-toolchain.toml`:
  ```toml
  [toolchain]
  channel = "stable"
  components = ["rustfmt", "clippy"]
  ```
- [x] Replace `.gitignore` (it is a Node template) with `/target`, `/pdf-output`, `/.hideo-cache`, `*.pdf` (except
      `!tests/**/*.pdf` if any are checked in), `.DS_Store`.
- [x] Delete `oxlint.json` (JavaScript linter config, no longer applicable).
- [x] README: `logo.png` is referenced but not tracked; add it or remove the line. Update the Semaphore badge
      and docs links to `semaphore.io` **(verify)**. Add install/usage sections (M9).
- [x] `CHANGELOG.md`: start a new section for the Rust rewrite.
- [x] Replace `.semaphore/semaphore.yml` with the pipeline in 8.6 **after** `Cargo.toml` exists.
- [x] Re-add `appveyor.yml` (the Node.js one was deleted in `a4405d4`) with the Rust version from 8.6; enable the (file is in; enabling the repository and the encrypted token are manual)
      repository in AppVeyor, encrypt the GitHub token (Account → Encrypt YAML) and paste it into `deploy`.
      Keep the Windows `RUSTFLAGS`/toolchain in sync with Semaphore via `rust-toolchain.toml`.
- [x] `renovate.json` currently has `automerge: true` for minor updates and `config:recommended`, which is
      wrong for exact-pinned Typst. Add:
  ```json
  "packageRules": [
    {
      "matchPackageNames": ["/^typst/"],
      "groupName": "typst",
      "automerge": false,
      "matchUpdateTypes": ["patch", "minor", "major"]
    },
    {
      "matchPackageNames": ["pulldown-cmark"],
      "automerge": false
    }
  ]
  ```
  (`minimumReleaseAge` stays.) The existing `commitBody` Signed-off-by line is kept.
- [x] Add `tests/fixtures/` skeletons (8.4) and the day-1 vertical-slice test: compile a hard-coded Typst string
      through `HideoWorld`, export with `typst_pdf::pdf`, load with `lopdf::Document::load_mem`, assert one page.
- [x] `hideo.toml` example from Appendix A at the repo root.
- [ ] `LICENSE` is MIT. All direct dependencies listed are MIT, Apache-2.0 or dual MIT/Apache-2.0 (`typst*`
      Apache-2.0, `pulldown-cmark` MIT, `mermaid-rs-renderer` MIT, `merman` MIT OR Apache-2.0), so this is
      compatible. Add `cargo-deny` **(optional)** with an allow-list to keep it that way, as a Semaphore job.
