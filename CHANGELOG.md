# Version history for Hideo (秀雄)

This changelog covers the version history and possible upcoming changes.
It follows the guidance from https://keepachangelog.com/en/1.0.0/.

## Unreleased

- **Breaking:** native flowchart and state diagram rendering is now enabled by default. Other
  diagram types use `gazu` by default; configure `[diagrams].command` to use another external
  renderer, or set `backend = "external"` to route every diagram through it.
- Cover subtitles can be overridden per tier with `[tier.<name>] subtitle = "..."`; the
  shared `[cover] subtitle` remains the fallback.
- The cover text panel has margins from the page edges and defaults to vertical centering at
  the lower golden cut.
- Warning codes can be suppressed in `[diagnostics]` with `suppress_warnings`, for example
  `suppress_warnings = ["W009"]` to omit warnings about non-descriptive link text.
- YAML frontmatter parsing uses `markdown-frontmatter` with only the YAML feature enabled.
- Frontmatter keys are documented: `title` and `hero` are required; `subtitle`, `author`,
  `lang`, and `hero_alt` are optional. Builds halt and generate no PDF for a tier when a
  required key is missing or empty.

## `v0.1.0` (2026-11-22)

- Single binary with `build`, `check` and `list` commands; the Typst compiler and PDF exporter
  are embedded.
- `--verbosity error|warning|info|debug` controls human-readable output; `-q` and `-v`
  remain aliases for `error` and `debug`.
- Strict Markdown profile with errors that name the page and line.

- Page-qualified heading labels, cross-page links, an unresolved-link report and
  `--deny-unresolved`.
- Cover, contents page with page numbers, hero images (scaled, cached, placeholders),
  tables with a repeating header row, lead-in paragraphs, external-link marker.
- Mermaid diagrams through an `mmdc`-compatible command, with a disk cache, a timeout and
  checks of the result, plus direct support for the `gazu` Pandoc-filter protocol;
  optional in-process renderer (`native-diagrams` feature) for flowchart and state diagrams.
- Tagged PDF with PDF/UA-1 validation on by default, table header cells (`TH`, column scope),
  decorative images as artifacts, alt text, document language and outline.
- Reproducible output from `SOURCE_DATE_EPOCH`.
- Gantt charts no longer give font notices: generic `font-family` attributes in diagrams are
  pointed at the embedded fonts.
