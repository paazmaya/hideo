// hideo template for Typst
// This file is embedded in the binary and used as the base template

#set page(width: {{PAGE_WIDTH_MM}}mm, height: 297mm, margin: (x: {{MARGIN_X_MM}}mm, y: {{MARGIN_Y_MM}}mm))

#set text(font: "Libertinus Serif", size: 10.5pt, fill: rgb("#2A2438"), lang: "{{BOOK_LANG}}")
#set par(leading: 0.75em, spacing: 0.65em, justify: false)

#set heading(numbering: none)
#set heading(1, size: 19pt, weight: 700, fill: rgb("#24134D"), above: 1.6em, below: 1.5em)
#set heading(2, size: 15pt, weight: 700, fill: rgb("#3D2A7A"), above: 2.4em, below: 1.4em)
#set heading(3, size: 12pt, weight: 700, fill: rgb("#52389A"), above: 1.8em, below: 1.1em)

#set list(spacing: 1.3em)

#set raw(block: false, size: 9pt, font: "DejaVu Sans Mono", fill: rgb("#F3F0F8"), radius: 4pt, inset: 9pt)

#show link: it => {
  set text(fill: rgb("#4A2F8F"))
  it
}

#show quote: it => {
  set block(
    width: 100%,
    fill: rgb("#F0EBF8"),
    inset: (x: 10pt, y: 8pt),
    stroke: (left: 2.5pt + rgb("#5B3FA0")),
    radius: (0, 3pt, 3pt, 0),
  )
  set par(spacing: 0.6em)
  set text(size: 10pt)
  v(1em)
  it
  v(1em)
}

#show rule: it => {
  set align(center)
  set text(size: 0)
  line(length: 30%, stroke: 0.6pt + rgb("#D3CCE4"))
  v(1.4em)
}

#let lead(text) = {
  set text(weight: 700, fill: rgb("#5B3FA0"))
  text
}

#let external-link-arrow = image("arrow.svg", height: 0.6em)

#show link.where(.dest != none): it => {
  let dest = it.dest
  if dest.starts_with("http") or dest.startsWith("//") or dest.contains(":") {
    it + " " + #external-link-arrow
  } else {
    it
  }
}

#set table(
  inset: (x: 4pt, y: 4pt),
  stroke: (x: 0pt, y: 0.4pt + rgb("#D3CCE4")),
  fill: (_, _) => none,
  columns: 1fr,
)

#show table: it => {
  set block(above: 1.8em, below: 1.8em)
  it
}

// Cover page
#set page(numbering: none)
#image("{{COVER_IMAGE}}", width: 100%, height: 100%)
#place(
  dx: {{PANEL_OFFSET_MM}}mm,
  dy: {{PANEL_OFFSET_MM}}mm,
  box(
    width: {{PANEL_WIDTH_MM}}mm,
    fill: rgb("#2E1A5C"),
    radius: 4pt,
    inset: 16pt,
    set text(fill: white)
    align(center)[
      #text(size: 24pt, weight: 700)[{{BOOK_TITLE}}]
      #v(8pt)
      #text(size: 16pt)[{{TIER_LINE}}]
      #v(8pt)
      #text(size: 12pt)[{{BOOK_AUTHOR}}]
    ]
  )
)

// Contents page
#set page(numbering: none)
#if "{{CONTENTS_HERO}}" != "" {
  #pad(x: -{{MARGIN_X_MM}}mm, top: -{{MARGIN_Y_MM}}mm)[#image("{{CONTENTS_HERO}}", width: 100%)]
  #v(5mm)
}
#set text(size: 19pt, weight: 700, fill: rgb("#24134D"))
Contents
#v(1.5em)
#set text(size: 7.5pt, leading: 0.4em)
#columns({{CONTENTS_COLUMNS}}, gutter: 6mm)[
  {{PAGE_INCLUDES}}
]

// Page footer with page numbers (except cover)
#show page: it => {
  set page(numbering: "1")
  set align(center)
  set text(size: 8pt, fill: rgb("#6E6682"))
  it
  if counter(page).get() > 1 {
    place(bottom + center, vy: -12pt)[
      context { counter(page).display() }
    ]
  }
}

// Hero image pulled into margins for content pages
#show heading.where(level: 1).where(
  it => query(selector("page").before(it)).len() > 0
): it => {
  it
}

// Main content pages
{{PAGE_INCLUDES}}