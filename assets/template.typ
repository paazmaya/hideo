// hideo template. Embedded in the binary; the double-brace tokens below are filled from the configuration.
// Imported by the generated main.typ and by every generated page as `/template.typ`.

#let margin-x = {{MARGIN_X_MM}}mm
#let margin-y = {{MARGIN_Y_MM}}mm
#let page-width = {{PAGE_WIDTH_MM}}mm

#let ink = rgb("#2A2438")
#let muted = rgb("#6E6682")
#let accent = rgb("#5B3FA0")
#let link-color = rgb("#4A2F8F")
#let rule-color = rgb("#D3CCE4")
#let h1-color = rgb("#24134D")
#let h2-color = rgb("#3D2A7A")
#let h3-color = rgb("#52389A")
#let panel-color = rgb("#2E1A5C")

// The whole book: document metadata, language, page geometry and styles.
#let book(lang: "en", title: "", author: "", body) = {
  set document(title: title, author: if author == "" { () } else { author })
  set text(lang: lang, font: ("Readex Pro", "Libertinus Serif"), size: 10.5pt, fill: ink)
  set page(
    width: page-width,
    height: 297mm,
    margin: (x: margin-x, y: margin-y),
    // Footer page number: centred, 8 pt. Typst marks footers as artifacts. The cover
    // (page 1) overrides the footer with `none`.
    footer: context {
      set text(size: 8pt, fill: muted)
      align(center, counter(page).display())
    },
  )
  set par(leading: 0.75em, spacing: 0.65em, justify: false)

  // Readex Pro has no italic and Typst does not fake one, so emphasis would be invisible.
  // Emphasised text uses the real italic of Libertinus Serif, scaled to match the larger x-height
  // of Readex Pro. The `emph` element stays, so it is still tagged as emphasis.
  show emph: set text(font: "Libertinus Serif", size: 1.1em)

  set heading(numbering: none)
  show heading.where(level: 1): set text(size: 19pt, weight: "bold", fill: h1-color)
  show heading.where(level: 1): set block(above: 1.6em, below: 1.5em)
  show heading.where(level: 2): set text(size: 15pt, weight: "bold", fill: h2-color)
  show heading.where(level: 2): set block(above: 2.4em, below: 1.4em)
  show heading.where(level: 3): set text(size: 12pt, weight: "bold", fill: h3-color)
  show heading.where(level: 3): set block(above: 1.8em, below: 1.1em)
  show heading.where(level: 4): set text(size: 10.5pt, weight: "bold", fill: h3-color)
  show heading.where(level: 4): set block(above: 1.6em, below: 1em)
  show heading.where(level: 5): set text(size: 10.5pt, weight: "bold", fill: h3-color)
  show heading.where(level: 5): set block(above: 1.6em, below: 1em)
  show heading.where(level: 6): set text(font: "Libertinus Serif", size: 11.5pt, style: "italic", weight: "regular", fill: h3-color)
  show heading.where(level: 6): set block(above: 1.6em, below: 1em)

  set list(spacing: 0.65em)
  show list: set block(below: 1.3em)
  show enum: set block(below: 1.3em)

  show raw: set text(font: "DejaVu Sans Mono", size: 8pt)
  show raw.where(block: true): it => block(
    width: 100%,
    fill: rgb("#F3F0F8"),
    inset: 9pt,
    radius: 4pt,
    breakable: true,
    it,
  )

  show link: set text(fill: link-color)

  show quote.where(block: true): it => block(
    width: 100%,
    fill: rgb("#F0EBF8"),
    inset: (x: 10pt, y: 8pt),
    stroke: (left: 2.5pt + accent),
    radius: (right: 3pt),
    above: 1em,
    below: 1em,
    {
      set text(size: 10pt)
      set par(spacing: 0.6em)
      it.body
    },
  )

  // Tables (Appendix B). Header rows come from `table.header(...)`, which is what makes them
  // `TH` cells in the tagged PDF; bold and fill here are only the look.
  set table(
    inset: 4pt,
    stroke: (x: none, y: 0.4pt + rule-color),
    fill: (_, y) => if y == 0 { rgb("#E9E3F6") } else if calc.even(y) { rgb("#F5F3F9") } else { none },
  )
  set table.cell(breakable: false)
  show table.header: set text(weight: "bold")
  show table: set text(hyphenate: true)
  show table: set block(above: 1.8em, below: 1.8em)

  body
}

// A thematic break: centred line, 30 % of the width.
#let hr() = align(center, block(above: 1.4em, below: 1.4em, line(length: 30%, stroke: 0.6pt + rule-color)))

// Lead-in label of a paragraph.
#let lead(body) = text(weight: "bold", fill: accent, body)

// External link followed by a decorative arrow icon. The link text identifies the destination;
// the icon must not create an additional Figure tag or be announced by assistive technology.
#let ext-link(url, body) = link(url)[#body#h(0.2em)#box(pdf.artifact(kind: "layout", image("/arrow.svg", height: 0.6em)))]

// Largest height an image or diagram may take: most of the text area, so a tall picture never
// outgrows a page (an unbreakable block taller than the page is clipped).
#let max-image-height = (297mm - 2 * margin-y) * 0.85

// A picture inside the text area. `upscale: true` scales it to the full text width (body
// photos); `false` never scales beyond its natural size (diagrams, whose text is sized by the
// diagram theme). Either way it is shrunk to fit the text width and `max-image-height`.
// `alt: none` marks it as decorative (an artifact).
#let fitted-image(path, alt, upscale) = block(width: 100%, above: 1em, below: 1em, breakable: false, layout(size => {
  let natural = measure(image(path))
  let factor = calc.min(
    size.width / natural.width,
    max-image-height / natural.height,
  )
  if not upscale { factor = calc.min(factor, 1) }
  let shown = if alt == none {
    pdf.artifact(kind: "layout", image(path, width: natural.width * factor))
  } else {
    image(path, width: natural.width * factor, alt: alt)
  }
  align(center, shown)
}))

// A body image scaled to the text width.
#let hideo-image(path, alt: none) = fitted-image(path, alt, true)

// A diagram: never larger than its natural size.
#let hideo-diagram(path, alt: none) = fitted-image(path, alt, false)

// The head of a content page: hero pulled into the margins, subtitle, author line.
// A hero is decorative (an artifact) unless it has alt text (decision D9).
#let page-head(hero: none, hero-alt: none, subtitle: none, author: none) = {
  if hero != none {
    let picture = if hero-alt == none {
      pdf.artifact(kind: "layout", image(hero, width: 100%))
    } else {
      image(hero, width: 100%, alt: hero-alt)
    }
    pad(x: -margin-x, top: -margin-y, picture)
    v(5mm)
  }
  if subtitle != none {
    text(size: 12pt, fill: muted, subtitle)
    v(8pt)
  }
  if author != none {
    text(size: 9pt, fill: muted, author)
    v(5mm)
  }
}

// Full-bleed cover: image, then a panel with title, optional subtitle and author.
#let cover(picture, title, subtitle, author, panel-width, panel-offset) = page(
  margin: 0pt,
  footer: none,
  {
    pdf.artifact(kind: "layout", image(picture, width: 100%, height: 100%, fit: "cover"))
    place(
      top + left,
      dx: panel-offset,
      dy: panel-offset,
      block(width: panel-width, fill: panel-color, radius: 4pt, inset: 16pt, {
        set text(fill: white)
        set par(spacing: 0.9em)
        text(size: 24pt, weight: "bold", title)
        if subtitle != "" {
          parbreak()
          text(size: 16pt, subtitle)
          parbreak()
        }
        text(size: 12pt, author)
      }),
    )
  },
)

// Native outline elements provide TOC/TOCI structure and correctly associate each entry's link
// with its destination.
#let contents(hero, title, columns-count) = {
  pagebreak(weak: true)
  if hero != none {
    pad(x: -margin-x, top: -margin-y, pdf.artifact(kind: "layout", image(hero, width: 100%)))
    v(5mm)
  }
  heading(level: 1, outlined: false, title)
  set text(size: 7.5pt)
  set par(leading: 0.4em)
  set outline.entry(fill: repeat[.])
  show outline.entry: set block(above: 0.55em, below: 0pt, width: 100%)
  columns(columns-count, gutter: 6mm, outline(title: none, target: heading.where(level: 1)))
}
