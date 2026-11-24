# hideo: open work

## First release (needs a tag)

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
- [ ] Run the real Mermaid CLI and veraPDF release checks documented in the README, and complete
      the screen-reader pass below.
- [ ] Publish the draft by hand once all five archives and their checksums are attached.
- [ ] Publish to crates.io with `cargo publish` (the Typst crates are exact-pinned, which is allowed for a
      binary crate). Run `cargo publish --dry-run` first and check the package size against the crates.io
      limit of 10 MB; the embedded fonts and test fixtures count towards it.

## Needs a person

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

## Optional

- [ ] **Table rows between 70 % and 100 % of a page** may split (`TALL_ROW_SHARE` in
      `src/markdown/tables.rs`), because the row height is only estimated. Tune the threshold on real
      content or accept it.
- [ ] **Emphasis typeface.** Readex Pro has no italic and Typst does not fake one, so `*emphasis*` is set in
      Libertinus Serif Italic inside sans-serif text. Keep it, or choose a family that has an italic.
- [ ] **`merman`** as another in-process Mermaid renderer: evaluate it on the nine diagram types and widen
      `NATIVE_KINDS` if it renders more of them well.
- [ ] **`--verbose`** prints only a summary line; add per-page and per-diagram detail.
- [ ] **Contact-sheet tool** that renders each diagram type through both backends for side-by-side review.
- [ ] **`deny.toml` exceptions.** RUSTSEC-2026-0194 and RUSTSEC-2026-0195 are ignored because `quick-xml`
      0.38 comes in through Typst's bibliography code. Remove the exceptions when a Typst release uses
      `quick-xml` 0.41 or newer, and run `cargo deny check` on every Typst upgrade.
