# Hideo (秀雄)

> Markdown to PDF book converter with many opinions baked in

[![Ubuntu Build Status](https://paazmaya.semaphoreci.com/badges/hideo/branches/main.svg)](https://paazmaya.semaphoreci.com/projects/hideo)

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

The design is specified in [PLAN.md](PLAN.md).

## Install

Building needs [Rust](https://www.rust-lang.org) 1.92 or newer, installed with
[rustup](https://rustup.rs).

```sh
cargo install --path .                      # Rust 1.92 or newer
cargo install --path . --features native-diagrams   # also render some diagrams in-process
```

## Quick start

```sh
hideo check  --input book            # validate, write nothing
hideo list   --input book            # pages in PDF order
hideo build  --input book --output pdf-output
```

`book/` contains a `hideo.toml` (see below), one folder per tier, and the images:

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
`--jobs N`, `-q` (no warnings), `-v`.

**Exit codes:** `0` success, `1` content or build error (a Markdown mistake names the page and
line), `2` usage or configuration error.

**Reproducible builds.** The PDF timestamp comes from
[`SOURCE_DATE_EPOCH`](https://reproducible-builds.org/docs/source-date-epoch/) (seconds since the
epoch, UTC), or 2000-01-01T00:00:00Z when it is unset. An invalid value is an error.

## Configuration (`hideo.toml`)

See the annotated [hideo.toml](hideo.toml). Unknown keys are an error. Paths are relative to the
config file, except `build.cache_dir`, which is relative to the output folder. Required keys:
`tiers`, `book.lang` (the document language, a [BCP 47](https://www.rfc-editor.org/info/bcp47) tag such as `fi` or `en-GB`: it is never
silently English) and `cover.title`.

## Markdown profile

Only a narrow, strict profile is supported; anything else is an error naming the page and line.

Supported: paragraphs, emphasis, strong, links, headings (with optional `{#custom-id}`),
bullet and numbered lists (nested), block quotes, fenced and indented code, thematic breaks,
tables, images (`![alt](path)`), Mermaid diagrams, and _lead-in_ paragraphs
(`**Note.** text`).

Not supported (errors): HTML blocks, footnotes, strikethrough, task lists, definition lists,
superscript and subscript. Inline HTML is dropped with a warning. `$` is always a literal dollar.

Links: relative links between pages become PDF links; `https://` and `mailto:` links are kept
(web links get a small arrow). Links into another tier cannot be followed in a separate PDF and
are reported as _outside this PDF_.

## Diagrams

A ` ```mermaid ` code block becomes an image. Give it alt text after the fence language:
` ```mermaid "How the pieces connect" `, or use `accTitle:` / `accDescr:` lines. Diagrams are
rendered by a command that follows the command line of `mmdc`
([mermaid-cli](https://github.com/mermaid-js/mermaid-cli), which needs
[Node.js](https://nodejs.org) and a Chromium browser), configured under
`[diagrams]`; [`mmdr`](https://crates.io/crates/mermaid-rs-renderer) works too. Results are cached, so only changed diagrams are rendered
again. On Windows use `command = "mmdc.cmd"` if `mmdc` is not found.

Built with `--features native-diagrams`, `backend = "native"` renders flowcharts and state
diagrams in-process (no Node.js or Chromium; it uses the
[`mermaid-rs-renderer`](https://crates.io/crates/mermaid-rs-renderer) crate) and hands the other types (sequence, class, ER, pie,
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
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo test --features native-diagrams
```

CI: [Semaphore](https://semaphoreci.com) (Linux, macOS) and
[AppVeyor](https://www.appveyor.com) (Windows); see PLAN.md section 8.6.

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
