use std::collections::HashMap;
use camino::Utf8PathBuf;
use crate::config::Config;
use crate::page::Page;
use crate::images::ProcessedImage;

pub struct Template {
    pub source: String,
}

impl Template {
    pub fn load() -> anyhow::Result<Self> {
        let source = include_str!("../../assets/template.typ").to_string();
        Ok(Self { source })
    }

    pub fn render_main(
        &self,
        config: &Config,
        tier: &str,
        pages: &[Page],
        cover_image: &ProcessedImage,
        contents_hero: Option<&ProcessedImage>,
        diagram_files: &HashMap<String, String>,
    ) -> String {
        let mut output = self.source.clone();

        output = output.replace("{{TIER}}", tier);
        output = output.replace("{{BOOK_TITLE}}", &config.cover.title);
        output = output.replace("{{BOOK_AUTHOR}}", &config.cover.author);
        output = output.replace("{{TIER_LINE}}", &config.cover.tier_line.replace("{Tier}", &tier.to_uppercase()));
        output = output.replace("{{PAGE_WIDTH_MM}}", &config.page.width_mm.to_string());
        output = output.replace("{{MARGIN_X_MM}}", &config.page.margin_x_mm.to_string());
        output = output.replace("{{MARGIN_Y_MM}}", &config.page.margin_y_mm.to_string());
        output = output.replace("{{HERO_HEIGHT_MM}}", &config.page.hero_height_mm.to_string());
        output = output.replace("{{CONTENTS_COLUMNS}}", &config.contents.columns.to_string());
        output = output.replace("{{COVER_IMAGE}}", &cover_image.virtual_path);
        output = output.replace("{{PANEL_WIDTH_MM}}", &config.cover.panel_width_mm.to_string());
        output = output.replace("{{PANEL_OFFSET_MM}}", &config.cover.panel_offset_mm.to_string());
        output = output.replace("{{BOOK_LANG}}", &config.book.lang);

        if let Some(contents_hero) = contents_hero {
            output = output.replace("{{CONTENTS_HERO}}", &contents_hero.virtual_path);
        } else {
            output = output.replace("{{CONTENTS_HERO}}", "");
        }

        let mut includes = String::new();
        for (i, page) in pages.iter().enumerate() {
            let page_num = i + 1;
            includes.push_str(&format!("#include \"pages/{:03}.typ\"\n", page_num));
        }
        output = output.replace("{{PAGE_INCLUDES}}", &includes);

        output
    }

    pub fn render_page(
        &self,
        config: &Config,
        page: &Page,
        hero_image: Option<&ProcessedImage>,
        page_num: usize,
        total_pages: usize,
        body_markup: &str,
    ) -> String {
        let mut output = String::new();

        output.push_str(&format!("#set page(width: {}mm, height: 297mm, margin: (x: {}mm, y: {}mm))\n",
            config.page.width_mm, config.page.margin_x_mm, config.page.margin_y_mm));

        if page_num == 1 {
            output.push_str("#set page(numbering: none)\n");
        }

        output.push_str(&format!("#set text(lang: \"{}\")\n", page.frontmatter.lang.as_deref().unwrap_or(&config.book.lang)));

        if let Some(hero) = hero_image {
            output.push_str(&format!("#pad(x: -{}mm, top: -{}mm)[#image(\"{}\", width: 100%)]\n",
                config.page.margin_x_mm, config.page.margin_y_mm, hero.virtual_path));
            output.push_str("#v(5mm)\n");
        }

        if let Some(subtitle) = &page.frontmatter.subtitle {
            output.push_str(&format!("#set text(size: 12pt, fill: rgb(\"6E6682\"))\n{}\n#v(8pt)\n", subtitle));
        }

        if let Some(author) = &page.frontmatter.author {
            output.push_str(&format!("#set text(size: 9pt, fill: rgb(\"6E6682\"))\n{}\n#v(5mm)\n", author));
        }

        output.push_str(body_markup);
        output.push('\n');

        if page_num > 1 {
            output.push_str(&format!("#set page(numbering: \"1\", number-format: \"1\")\n"));
            output.push_str("#set align(center)\n#set text(size: 8pt, fill: rgb(\"6E6682\"))\n");
            output.push_str(&format!("#context {{ counter(page).display() }}\n"));
        }

        output
    }

    pub fn render_cover(&self, config: &Config, tier: &str, cover_image: &ProcessedImage) -> String {
        let mut output = String::new();

        output.push_str(&format!("#set page(width: {}mm, height: 297mm, margin: 0, numbering: none)\n",
            config.page.width_mm));

        output.push_str(&format!("#image(\"{}\", width: 100%, height: 100%)\n", cover_image.virtual_path));

        output.push_str(&format!("#place(
  dx: {}mm,
  dy: {}mm,
  #box(
    width: {}mm,
    fill: rgb(\"2E1A5C\"),
    radius: 4pt,
    inset: 16pt,
    #set text(fill: white)
    #align(center)[
      #text(size: 24pt, weight: 700)[{}]\n",
            config.cover.panel_offset_mm,
            config.cover.panel_offset_mm,
            config.cover.panel_width_mm,
            config.cover.title));

        output.push_str(&format!("      #v(8pt)\n      #text(size: 16pt)[{}]\n", config.cover.tier_line.replace("{Tier}", &tier.to_uppercase())));
        output.push_str(&format!("      #v(8pt)\n      #text(size: 12pt)[{}]\n", config.cover.author));
        output.push_str("    ]\n  )\n)\n");

        output
    }

    pub fn render_contents(&self, config: &Config, pages: &[Page], contents_hero: Option<&ProcessedImage>) -> String {
        let mut output = String::new();

        output.push_str(&format!("#set page(width: {}mm, height: 297mm, margin: (x: {}mm, y: {}mm))\n",
            config.page.width_mm, config.page.margin_x_mm, config.page.margin_y_mm));
        output.push_str("#set page(numbering: none)\n");
        output.push_str(&format!("#set text(lang: \"{}\")\n", config.book.lang));

        if let Some(hero) = contents_hero {
            output.push_str(&format!("#pad(x: -{}mm, top: -{}mm)[#image(\"{}\", width: 100%)]\n",
                config.page.margin_x_mm, config.page.margin_y_mm, hero.virtual_path));
            output.push_str("#v(5mm)\n");
        }

        output.push_str("#set text(size: 19pt, weight: 700, fill: rgb(\"24134D\"))\n");
        output.push_str("Contents\n");
        output.push_str("#v(1.5em)\n");

        output.push_str(&format!("#set text(size: 7.5pt, leading: 0.4em)\n"));
        output.push_str(&format!("#columns({}, gutter: 6mm)[\n", config.contents.columns));

        let mut current_folder = String::new();
        for (i, page) in pages.iter().enumerate() {
            let folder = page.path.parent().map(|p| p.to_string()).unwrap_or_default();
            if folder != current_folder && !folder.is_empty() {
                if !current_folder.is_empty() {
                    output.push_str("#v(1em)\n");
                }
                current_folder = folder.clone();
                output.push_str(&format!("#set text(size: 8.5pt, weight: 700, fill: rgb(\"3D2A7A\"))\n{}", folder));
                output.push_str("#v(0.5em)\n");
            }

            let page_num = i + 1;
            let title = page.frontmatter.title.as_deref().unwrap_or("Untitled");
            let page_label = format!("<{}>", page.key);
            output.push_str(&format!("#h(0.55em)[#link({})[{}]]\n", page_label, title));
        }

        output.push_str("]\n");

        output
    }
}