use std::collections::HashMap;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub tiers: Vec<String>,
    #[serde(default = "default_ignore_prefixes")]
    pub ignore_prefixes: Vec<String>,

    #[serde(default)]
    pub page: PageConfig,
    #[serde(default)]
    pub images: ImagesConfig,
    #[serde(default)]
    pub cover: CoverConfig,
    #[serde(default)]
    pub contents: ContentsConfig,
    #[serde(default)]
    pub diagrams: DiagramsConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,

    pub book: BookConfig,
    #[serde(default)]
    pub accessibility: AccessibilityConfig,
}

fn default_ignore_prefixes() -> Vec<String> {
    vec!["_".to_string(), ".".to_string()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageConfig {
    #[serde(default = "default_page_width_mm")]
    pub width_mm: f32,
    #[serde(default = "default_margin_x_mm")]
    pub margin_x_mm: f32,
    #[serde(default = "default_margin_y_mm")]
    pub margin_y_mm: f32,
    #[serde(default = "default_hero_height_mm")]
    pub hero_height_mm: f32,
}

fn default_page_width_mm() -> f32 { 210.0 }
fn default_margin_x_mm() -> f32 { 20.0 }
fn default_margin_y_mm() -> f32 { 18.0 }
fn default_hero_height_mm() -> f32 { 118.125 }

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            width_mm: default_page_width_mm(),
            margin_x_mm: default_margin_x_mm(),
            margin_y_mm: default_margin_y_mm(),
            hero_height_mm: default_hero_height_mm(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImagesConfig {
    #[serde(default = "default_dpi")]
    pub dpi: u32,
    #[serde(default = "default_jpeg_quality")]
    pub jpeg_quality: u8,
    pub contents_hero: Option<String>,
    #[serde(default = "default_cover_pattern")]
    pub cover_pattern: String,
}

fn default_dpi() -> u32 { 200 }
fn default_jpeg_quality() -> u8 { 70 }
fn default_cover_pattern() -> String { "images/cover_front_{tier}.png".to_string() }

impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            dpi: default_dpi(),
            jpeg_quality: default_jpeg_quality(),
            contents_hero: None,
            cover_pattern: default_cover_pattern(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverConfig {
    pub title: String,
    pub author: String,
    #[serde(default = "default_tier_line")]
    pub tier_line: String,
    #[serde(default = "default_panel_width_mm")]
    pub panel_width_mm: f32,
    #[serde(default = "default_panel_offset_mm")]
    pub panel_offset_mm: f32,
}

fn default_tier_line() -> String { "{Tier} course material".to_string() }
fn default_panel_width_mm() -> f32 { 170.0 }
fn default_panel_offset_mm() -> f32 { 20.0 }

impl Default for CoverConfig {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            tier_line: default_tier_line(),
            panel_width_mm: default_panel_width_mm(),
            panel_offset_mm: default_panel_offset_mm(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentsConfig {
    #[serde(default = "default_contents_columns")]
    pub columns: usize,
}

fn default_contents_columns() -> usize { 2 }

impl Default for ContentsConfig {
    fn default() -> Self {
        Self {
            columns: default_contents_columns(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagramsConfig {
    #[serde(default = "default_diagrams_backend")]
    pub backend: DiagramBackendConfig,
    #[serde(default = "default_diagrams_command")]
    pub command: String,
    #[serde(default = "default_diagrams_args")]
    pub args: Vec<String>,
    #[serde(default = "default_diagrams_timeout")]
    pub timeout_s: u64,
    pub theme: String,
}

fn default_diagrams_backend() -> DiagramBackendConfig { DiagramBackendConfig::External }
fn default_diagrams_command() -> String { "mmdc".to_string() }
fn default_diagrams_args() -> Vec<String> {
    vec![
        "-i".to_string(),
        "{input}".to_string(),
        "-o".to_string(),
        "{output}".to_string(),
        "-c".to_string(),
        "{theme}".to_string(),
        "-e".to_string(),
        "svg".to_string(),
    ]
}
fn default_diagrams_timeout() -> u64 { 30 }

impl Default for DiagramsConfig {
    fn default() -> Self {
        Self {
            backend: default_diagrams_backend(),
            command: default_diagrams_command(),
            args: default_diagrams_args(),
            timeout_s: default_diagrams_timeout(),
            theme: "assets/mermaid-theme.json".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagramBackendConfig {
    Native,
    External,
    Off,
}

impl Default for DiagramBackendConfig {
    fn default() -> Self {
        Self::External
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
}

fn default_cache_dir() -> String { ".hideo-cache".to_string() }

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticsConfig {
    #[serde(default)]
    pub warnings_as_errors: bool,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            warnings_as_errors: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BookConfig {
    pub lang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilityConfig {
    #[serde(default = "default_true")]
    pub tagged: bool,
    #[serde(default = "default_true")]
    pub pdf_ua: bool,
}

fn default_true() -> bool { true }

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            tagged: true,
            pdf_ua: true,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.tiers.is_empty() {
            errors.push("tiers must not be empty".to_string());
        }

        let mut seen = HashMap::new();
        for (i, tier) in self.tiers.iter().enumerate() {
            if tier.contains('/') || tier.contains('\\') {
                errors.push(format!("tier '{}' must not contain path separators", tier));
            }
            if let Some(first) = seen.get(tier) {
                errors.push(format!("duplicate tier '{}' (first at index {})", tier, first));
            } else {
                seen.insert(tier, i);
            }
        }

        if self.page.width_mm <= 2.0 * self.page.margin_x_mm {
            errors.push("page.width_mm must be greater than 2 * page.margin_x_mm".to_string());
        }

        if !(1..=100).contains(&self.images.jpeg_quality) {
            errors.push("images.jpeg_quality must be between 1 and 100".to_string());
        }

        if !(72..=600).contains(&self.images.dpi) {
            errors.push("images.dpi must be between 72 and 600".to_string());
        }

        if self.accessibility.pdf_ua && !self.accessibility.tagged {
            errors.push("accessibility.pdf_ua requires accessibility.tagged = true".to_string());
        }

        if self.book.lang.is_empty() {
            errors.push("book.lang is required".to_string());
        } else if !is_valid_bcp47(&self.book.lang) {
            errors.push(format!("book.lang '{}' is not a valid BCP 47 tag", self.book.lang));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn images_contents_hero_path(&self) -> Option<Utf8PathBuf> {
        self.images.contents_hero.as_ref().map(|s| Utf8PathBuf::from(s))
    }

    pub fn images_cover_pattern(&self) -> &str {
        &self.images.cover_pattern
    }

    pub fn diagrams_theme_path(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(&self.diagrams.theme)
    }

    pub fn build_cache_dir(&self) -> Utf8PathBuf {
        Utf8PathBuf::from(&self.build.cache_dir)
    }
}

fn is_valid_bcp47(tag: &str) -> bool {
    let parts: Vec<&str> = tag.split('-').collect();
    if parts.is_empty() || parts.len() > 3 {
        return false;
    }
    let lang = parts[0];
    if lang.len() != 2 && lang.len() != 3 {
        return false;
    }
    if !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    for part in &parts[1..] {
        if part.len() != 2 && part.len() != 4 {
            return false;
        }
        if !part.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}