# Version history for Hideo (秀雄)

This changelog covers the version history and possible upcoming changes.
It follows the guidance from https://keepachangelog.com/en/1.0.0/.

## Unreleased `v0.1.0` (2026-11-20)

The project is rewritten in Rust (the previous Node.js implementation was removed in
`a4405d4`). See [PLAN.md](PLAN.md) for the design.

### Added

- Single binary with `build`, `check` and `list` commands; the Typst compiler and PDF exporter
  are embedded.
- Strict Markdown profile with errors that name the page and line.
- Page-qualified heading labels, cross-page links, an unresolved-link report and
  `--deny-unresolved`.
- Cover, contents page with page numbers, hero images (scaled, cached, placeholders),
  tables with a repeating header row, lead-in paragraphs, external-link marker.
- Mermaid diagrams through an `mmdc`-compatible command, with a disk cache, a timeout and
  checks of the result; optional in-process renderer (`native-diagrams` feature) for
  flowchart and state diagrams.
- Tagged PDF with PDF/UA-1 validation on by default, table header cells (`TH`, column scope),
  decorative images as artifacts, alt text, document language and outline.
- Reproducible output from `SOURCE_DATE_EPOCH`.
- CI definitions for Semaphore (Linux, macOS) and AppVeyor (Windows), including a
  `cargo deny` check of licenses and advisories (`deny.toml`; cargo-deny is installed with
  `cargo binstall`).
- Gantt charts no longer give font notices: generic `font-family` attributes in diagrams are
  pointed at the embedded fonts.

### Removed

- The Node.js tool, `oxlint.json`.
