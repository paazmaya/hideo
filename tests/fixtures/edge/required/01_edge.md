---
title: "Edge Cases"
hero: "hero.png"
---

# Unicode Headings

## ÄÖÅ Unicode

Content with unicode.

## 日本語

Japanese content.

---

# Long Table Strings

| Header | VeryLongColumnNameThatShouldBeWrapped |
|--------|---------------------------------------|
| Cell   | ThisIsAVeryLongStringWithoutSpacesThatShouldBeBrokenByZeroWidthSpaces |

---

# Missing Hero

---
title: "Missing Hero"
---

This page has no hero image.

---

# Unresolved Links

Link to [missing page](missing.md) and [missing anchor](#nonexistent).

---

# Pathological Markdown

* a_
* a_
* a_

[^
[^
[^

Footnote reference [^1].

[^1]: Footnote definition.