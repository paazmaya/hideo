# Hideo (秀雄)

> Markdown to PDF book converter with many opinions baked in for full accessibility

[![Semaphore CI status](https://paazmaya.semaphoreci.com/badges/hideo/branches/main.svg)](https://paazmaya.semaphoreci.com/projects/hideo)
[![Appveyor CI status](https://ci.appveyor.com/api/projects/status/ix6ou9ex448eubq1/branch/main?svg=true)](https://ci.appveyor.com/project/paazmaya/hideo/branch/main)

`hideo` turns a folder of Markdown pages and images into one PDF book per _tier_
(a tier is a folder such as `required`, `additional`, `bonus`). It is a single binary: the
[Typst](https://typst.app) compiler and PDF exporter are embedded, so there is nothing else
to install. The only optional external dependency is a renderer for
[Mermaid](https://mermaid.js.org) diagrams.

What you get: a full-bleed cover, a contents page with page numbers, one hero image per
page, working internal links, styled tables whose header row repeats across pages,
Mermaid diagrams, and an **accessible, tagged PDF that is validated
against [PDF/UA-1](https://pdfa.org/resource/iso-14289-pdfua/)** on every build. Two builds of
the same sources are byte-identical.

The resulting PDF can then be tested for example with [PAC](https://pac.pdf-accessibility.org/en), that has been very helpful in making `hideo` to produce fully accessible PDF documents.

What is still to do is listed in [PLAN.md](PLAN.md).

## Install

Building needs [Rust](https://www.rust-lang.org) 1.92 or newer, installed with
[rustup](https://rustup.rs).

```sh
cargo install --path .                      # Rust 1.92 or newer
cargo install --path . --features native-diagrams   # also render some diagrams in-process
```

## Quick start

1. Create a folder for the book and copy the annotated [hideo.toml](hideo.toml) from this
   repository into it. The file lists every option with its default value and the alternatives
   in comments, so delete what you do not need. Only three settings have no default and must
   be set: `tiers`, `book.lang` and `cover.title`.
2. Add one folder per tier with the pages, and the images (layout below).
3. Run:

```sh
hideo check  --input book            # validate, write nothing
hideo list   --input book            # pages in PDF order
hideo build  --input book --output pdf-output
```

`book/` contains a `hideo.toml` (see [Configuration](#configuration-hideotoml)), one folder per
tier, and the images:

```text
book/
  hideo.toml
  images/hero.png
  required/
    01_intro.md
    02_setup/01_install.md
  additional/
    01_more.md
```

Every page starts with a small frontmatter block:

```markdown
---
title: "Introduction" # required, shown in the contents
hero: "images/hero.png" # required; next to the page first, then relative to the book folder
subtitle: "Getting started" # optional
author: "A. Author" # optional
lang: "en" # optional: language of this page when it differs from book.lang
hero_alt: "A mountain at dawn" # optional: makes the hero real content with alt text
---

# Welcome

Body in the Markdown profile below.
```

The frontmatter keys are:

| Key        | Required | Purpose                                                                          |
| ---------- | -------- | -------------------------------------------------------------------------------- |
| `title`    | Yes      | Page title shown in the contents and PDF metadata.                               |
| `hero`     | Yes      | Hero image path, resolved next to the page and then relative to the book folder. |
| `subtitle` | No       | Subtitle shown on the page.                                                      |
| `author`   | No       | Author name shown on the page.                                                   |
| `lang`     | No       | Page language override when it differs from `book.lang`.                         |
| `hero_alt` | No       | Alternate text that makes the hero image meaningful PDF content.                 |

A build is halted when any required frontmatter key is missing or empty. No PDF is generated for the affected tier.

The configuration is a [TOML](https://toml.io) file. Pages are ordered by file path with a case-insensitive natural sort (`2_a.md` before
`10_a.md`). Files and folders whose names start with `_` or `.` are ignored.

## Commands

| Command                 | What it does                                                                                                                                                                     |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hideo build [TIER...]` | Build one PDF per tier into `--output` (default `pdf-output`).                                                                                                                   |
| `hideo check [TIER...]` | Validate everything that does not need Typst: frontmatter, links, headings, tables, images, diagram sources. `--format json` prints `{"errors":[…],"warnings":[…],"stats":{…}}`. |
| `hideo list [TIER...]`  | Print the pages in PDF order (`--format json` available).                                                                                                                        |

Useful `build` flags: `--diagrams native|external|off`, `--keep-typst DIR` (write the generated
Typst for debugging), `--deny-unresolved` (exit 1 if an internal link does not resolve),
`--jobs N`, `-q` (no warnings), `-v` (debug verbosity).

Build mode is fail-fast: rendering or compilation stops at the first error, and subsequent
warnings and unresolved-link summaries are omitted. `check` mode still collects all diagnostics.

`--verbosity error|warning|info|debug` controls human-readable output for all commands. Each level
includes more severe messages: `error` prints only errors, `warning` adds warnings, `info` also
prints normal command output (the default), and `debug` adds build timing and statistics. JSON
output remains complete for machine consumers. The legacy `-q` and `-v` flags are aliases for the
`error` and `debug` levels.

**Exit codes:** `0` success, `1` content or build error (a Markdown mistake names the page and
line), `2` usage or configuration error.

**Reproducible builds.** The PDF timestamp comes from
[`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/docs/source-date-epoch/) (seconds since the
epoch, UTC), or 2000-01-01T00:00:00Z when it is unset. An invalid value is an error.

## Configuration (`hideo.toml`)

The annotated [hideo.toml](hideo.toml) in the root of this repository is the full reference: every
option, its default, the allowed range and the alternatives. Use it as the starting point for
your own file. Anything you leave out falls back to the default.

| Table             | What it controls                                  |
| ----------------- | ------------------------------------------------- |
| (top level)       | `tiers`, `ignore_prefixes`                        |
| `[book]`          | document language                                 |
| `[cover]`         | title, author, tier line, text panel size         |
| `[contents]`      | heading and number of columns                     |
| `[page]`          | page width and margins                            |
| `[images]`        | `dpi`, `jpeg_quality`, cover and contents images  |
| `[diagrams]`      | Mermaid backend, renderer command, timeout, theme |
| `[build]`         | cache folder                                      |
| `[diagnostics]`   | `warnings_as_errors`, `suppress_warnings`         |
| `[accessibility]` | tagged PDF and PDF/UA validation                  |

Unknown keys are an error. Paths are relative to the config file, except `build.cache_dir`,
which is relative to the output folder. Required keys: `tiers`, `book.lang` (the document
language, a [BCP 47](https://www.rfc-editor.org/info/bcp47) tag such as `fi` or `en-GB`: it is
never silently English) and `cover.title`. A configuration error exits with code 2.

## Markdown profile

Only a narrow, strict profile is supported; anything else is an error naming the page and line.

Supported: paragraphs, emphasis, strong, links, headings (with optional `{#custom-id}`),
bullet and numbered lists (nested), block quotes, fenced and indented code, thematic breaks,
tables, images (`![alt](path)`), Mermaid diagrams, and _lead-in_ paragraphs
(`**Note.** text`).

Not supported (errors): HTML blocks, footnotes, strikethrough, task lists, definition lists,
superscript and subscript. Inline HTML is dropped with a warning. `$` is always a literal dollar.

Links: relative links between pages become PDF links; `https://` and `mailto:` links are kept
(web links get a small arrow). Bare email addresses become `mailto:` links by default; set
`links.auto_mailto = false` to keep them as plain text. Links into another tier cannot be followed
in a separate PDF and are reported as _outside this PDF_.

## Diagrams

A ` ```mermaid ` code block becomes an image. Give it alt text after the fence language:
` ```mermaid "How the pieces connect" `, or use `accTitle:` / `accDescr:` lines. Diagrams are
rendered by a command that follows the command line of `mmdc`
([mermaid-cli](https://github.com/mermaid-js/mermaid-cli), which needs
[Node.js](https://nodejs.org) and a Chromium browser), configured under
`[diagrams]`; [`mmdr`](https://github.com/1jehuang/mermaid-rs-renderer) and
[`gazu`](https://crates.io/crates/gazu) work too. For `gazu`, hideo speaks its
Pandoc-filter protocol, passes the Mermaid theme through `GAZU_CONFIG`, and accepts the
default `mmdc`-shaped `args` unchanged. Results are cached, so only changed diagrams are rendered
again. On Windows use `command = "mmdc.cmd"` if `mmdc` is not found.
Custom Mermaid themes inherit `Readex Pro` as `fontFamily` at the root and under
`themeVariables` when those fields are omitted.

Built with `--features native-diagrams`, `backend = "native"` renders flowcharts and state
diagrams in-process (no Node.js or Chromium; it uses the
[`mermaid-rs-renderer`](https://github.com/1jehuang/mermaid-rs-renderer) crate) and hands the other types (sequence, class, ER, pie,
mindmap, gantt, ...) to the external command.

## Accessibility

Every PDF is a tagged PDF and is validated against **PDF/UA-1** by default
(`accessibility.pdf_ua = true`; a violation fails the build and names the page and line).
Table header rows are real header cells with column scope, decorative images (heroes, the
cover, the contents image, the dotted leaders) are marked as artifacts, every other image has
alt text, the language is set, and headings produce the PDF outline. `check` also warns about
skipped heading levels, empty table headers, non-descriptive link text and diagrams without alt text.

Typst's validation is structural: it does not judge whether alt text is useful, whether the
reading order makes sense, or colour contrast (the palette is tested separately). It is not a
certification. The output of the test fixtures also passes [veraPDF](https://verapdf.org)
(`--flavour ua1`); a screen reader pass is still open, see PLAN.md.

Characters that the embedded fonts do not cover (for example Japanese) cannot be displayed;
the build reports them as errors, and `check` warns first.

## Fonts

Only the fonts embedded in the binary are used, so output never depends on the machine.
The default text font is [Readex Pro](https://github.com/ThomasJockin/readexpro) (SIL Open Font
License 1.1; the font files and the license are in
[assets/fonts/readexpro/](assets/fonts/readexpro/)). It has no italic, and Typst does not fake
one, so emphasis (`*text*`) is set in the italic of
[Libertinus Serif](https://github.com/alerque/libertinus). Libertinus Serif also supplies glyphs
that Readex Pro lacks. Code uses [DejaVu Sans Mono](https://dejavu-fonts.github.io), and Typst
also embeds [New Computer Modern](https://ctan.org/pkg/newcomputermodern). Readex Pro covers
Latin (the Finnish and Swedish letters render, see the test fixtures) but not Japanese; see the
character coverage note under Accessibility.

## Development

```sh
cargo test                       # unit, property and end-to-end tests
cargo nextest run                # faster parallel test execution with isolated processes
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo test --features native-diagrams
cargo deny check                 # licenses and security advisories, see below
```

In CI, tests run with [cargo-nextest](https://nexte.st) using the `ci` profile configured in
[`.config/nextest.toml`](.config/nextest.toml):

```sh
cargo nextest run --profile ci
```

This exports structured JUnit XML test reports to `target/nextest/ci/junit.xml`. In Semaphore CI,
`test-results publish target/nextest/ci/junit.xml` runs in the task's `epilogue.always` block so that
individual test suites, case execution times, and failure messages are always recorded and visible in
the Semaphore CI test dashboard regardless of whether tests pass or fail.

`cargo deny check` needs [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), which is not
part of Rust. Install it once with `cargo install cargo-deny --locked`, or download a prebuilt
binary with [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):
`cargo binstall cargo-deny`. The rules are in [deny.toml](deny.toml): only permissive licenses are
allowed, and the two advisories that cannot be fixed here (they sit inside Typst's bibliography
code, which `hideo` does not use) are listed there with the reason.

```sh
winget install WiXToolset.WiXToolset
cargo install cargo-wix
cargo wix -b "C:\Program Files (x86)\WiX Toolset v3.14\bin"
```

Cross compile to Windows MSVC target when not using Windows:

```sh
cargo install --locked cargo-xwin
cargo xwin run --target x86_64-pc-windows-msvc
```

## Background for the project name

The name of the project is for honouring the legacy of Mrs Sonobe Hideo (園部 秀雄),
who was the 15th head master of
[Jikishinkageryu Naginatajutsu (直心影流薙刀術)](https://naginata.fi/en/koryu),
which is an ancient Japanese martial art, focusing the handling of a long pole like weapon
called naginata.

[Read more about why these martial arts are important for me at `karatejukka.fi`.](https://karatejukka.fi)

## Version history

[Changes happening across different versions and upcoming changes are tracked in the `CHANGELOG.md` file.](CHANGELOG.md)

## License

Copyright (c) [Juga Paazmaya](https://paazmaya.fi) <paazmaya@yahoo.com>

Licensed under [the MIT license](./LICENSE).
