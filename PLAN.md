# hideo: open work

Last updated 2026-11-20. This file lists only what is still to do. Everything in section 1 to 3 needs
something that cannot be done from a local session: a push to GitHub, a version tag, or a person.
Usage and behaviour are described in [README.md](README.md).

## 1. First CI run (needs a push)

### Semaphore (`.semaphore/semaphore.yml`)

- [ ] The pipeline starts. The `change_in([...], {default_branch: 'main'})` conditions are accepted, jobs
      run for changes under `src/`, `tests/`, `assets/` and the Cargo files, and a docs-only commit starts
      none of them.
- [ ] The log shows `rustc -V`. If Rust is preinstalled on the image, delete the rustup install line from
      the prologue of both Semaphore files (`rust-toolchain.toml` picks the toolchain either way).
- [ ] Every machine type is accepted. The organization is on the open source plan and refused `f1-standard-2`,
      so the Linux x86 jobs use `e1-standard-2` (2 vCPU, 4 GB, 25 GB disk). Confirm that `r1-standard-2`
      (ARM) and `a2-standard-4` (macOS) are accepted too; if one is refused, drop its block or ask Semaphore
      support to enable it. `os_image: macos-xcode26` is unconfirmed; correct it from the workflow editor if
      Semaphore rejects it. Try `e1-standard-4` if the Linux job is too slow or runs out of memory.
- [ ] The Linux block (fmt, clippy, test, fixture built twice with identical PDFs), MSRV (Rust 1.92), and, on
      `main` and tags, Test (Linux ARM) and Test (macOS) all pass.
- [ ] The `cache delete` plus `cache store` pattern with a fallback key works: a second run with a changed
      `Cargo.lock` restores the fallback copy and recompiles only what changed.
- [ ] The "Dependencies" block (a separate cheap job, no compile) installs `cargo-deny` with `cargo binstall` and `cargo deny check` passes. If
      the binstall install script fails on the image, fall back to `cargo install cargo-deny --locked`.
- [ ] Compare the cost of restoring and storing the `target` caches (Linux, ARM, macOS) with the compile time
      they save. If one costs more, keep only the registry cache for that platform.
- [ ] Check the minutes used per push against the open source plan's allowance, and revisit the rule that
      limits the ARM and macOS jobs to `main` and tags.
- [ ] The README badge reads "passing".

### AppVeyor (`appveyor.yml`)

- [ ] Enable the repository in AppVeyor. Encrypt a GitHub token (Account, Encrypt YAML) and replace
      `<encrypted GitHub token>` in the `deploy` section.
- [ ] The log shows whether rustup is preinstalled on the `Visual Studio 2022` image. If it is, delete the
      install step.
- [ ] Record the job duration on the 2-core VM. If it approaches the plan's per-job limit, try the 4-core
      configuration, or move further steps to Semaphore.
- [ ] Check that restoring and saving the `%USERPROFILE%\.cargo\registry\cache` entry is faster than
      downloading the crates. If not, remove the entry.
- [ ] The Windows run passes, including the `windows-paths` fixture and the reproducibility check.

## 2. First release (needs a tag)

Tag a commit that changed code. `skip_commits` on AppVeyor would skip a tag that points at a docs-only
commit.

- [ ] Create the `GH_TOKEN` secret in Semaphore (`sem create secret`); `release.yml` reads it.
- [ ] The `vX.Y.Z` tag starts the Semaphore release promotion. Four archives are built, checksummed, pulled
      by the final job and uploaded to a draft release: `x86_64-unknown-linux-gnu`,
      `aarch64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`.
- [ ] The tag also starts the AppVeyor build. It produces `hideo-<tag>-x86_64-pc-windows-msvc.zip` and its
      `.sha256`, and deploys them to the same draft release.
- [ ] Both systems create or update the same draft. Check that they do not collide when they finish at the
      same time. If they do, let one of them be the only creator of the release.
- [ ] Decide the checksum layout: one `.sha256` file per archive (current) or a single `SHA256SUMS`.
- [ ] Download one archive per platform. Verify the checksum, run `hideo --version`, and build
      `tests/fixtures/full`.
- [ ] Publish the draft by hand once all five archives and their checksums are attached.
- [ ] Publish to crates.io with `cargo publish` (the Typst crates are exact-pinned, which is allowed for a
      binary crate). Run `cargo publish --dry-run` first and check the package size against the crates.io
      limit of 10 MB; the embedded fonts and test fixtures count towards it.

## 3. Needs a person

- [ ] **Screen-reader pass with NVDA.** Cover, contents page and a table-heavy page of a built PDF. Check
      that headings and the outline navigate, that table header cells are announced with their column, that
      decorative images (heroes, cover) are skipped, that diagrams are read with their alt text, and that
      the reading order is sensible. Record what was found. The PDF/UA validation in the build is
      structural only and cannot judge any of this.
- [ ] **The author's real book.** Build it with `mmdc` and look for diagram errors (E004), font notices
      (W010), timeouts, and characters the embedded fonts do not cover (W011). Readex Pro covers Latin but
      not Japanese; if the book needs more scripts, a font has to be added and embedded.
- [ ] **Paragraph spacing.** The template uses paragraph spacing of 0.65 em and leading of 0.75 em. In Typst
      both are gaps, so paragraphs end up closer together than the lines inside them. Confirm this is the
      intended look or change the values.
- [ ] **Cover text.** Confirm the wording of the cover title, author and line template in `hideo.toml`.
- [ ] Check the `pdfa.org` link in the README by hand; it answers 403 to scripts.

## 4. Optional

- [ ] **Table rows between 70 % and 100 % of a page** may split (`TALL_ROW_SHARE` in
      `src/markdown/tables.rs`), because the row height is only estimated. Tune the threshold on real
      content or accept it.
- [ ] **Emphasis typeface.** Readex Pro has no italic and Typst does not fake one, so `*emphasis*` is set in
      Libertinus Serif Italic inside sans-serif text. Keep it, or choose a family that has an italic.
- [ ] **`merman`** as another in-process Mermaid renderer: evaluate it on the nine diagram types and widen
      `NATIVE_KINDS` if it renders more of them well.
- [ ] **`--verbose`** prints only a summary line; add per-page and per-diagram detail.
- [ ] **Contact-sheet tool** that renders each diagram type through both backends for side-by-side review.
- [ ] **veraPDF in CI** (needs Java and its installer), or as a manual check before each release.
- [ ] **`deny.toml` exceptions.** RUSTSEC-2026-0194 and RUSTSEC-2026-0195 are ignored because `quick-xml`
      0.38 comes in through Typst's bibliography code. Remove the exceptions when a Typst release uses
      `quick-xml` 0.41 or newer, and run `cargo deny check` on every Typst upgrade.
