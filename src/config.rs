//! Configuration file (`hideo.toml`), see plan 7.4 and Appendix A.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::error::ConfigError;

/// Default diagram theme, embedded so the binary works without the `assets/` folder.
pub const DEFAULT_DIAGRAM_THEME: &str = include_str!("../assets/mermaid-theme.json");

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

    /// Required: the document language is never silently English (decision D11).
    pub book: BookConfig,
    #[serde(default)]
    pub accessibility: AccessibilityConfig,

    /// Directory of the config file; relative paths in the file resolve against it.
    #[serde(skip)]
    pub base_dir: Utf8PathBuf,
}

fn default_ignore_prefixes() -> Vec<String> {
    vec!["_".to_string(), ".".to_string()]
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct PageConfig {
    pub width_mm: f64,
    pub margin_x_mm: f64,
    pub margin_y_mm: f64,
    pub hero_height_mm: f64,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            width_mm: 210.0,
            margin_x_mm: 20.0,
            margin_y_mm: 18.0,
            hero_height_mm: 118.125,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ImagesConfig {
    pub dpi: u32,
    pub jpeg_quality: u8,
    pub contents_hero: Option<String>,
    pub cover_pattern: String,
}

impl Default for ImagesConfig {
    fn default() -> Self {
        Self {
            dpi: 200,
            jpeg_quality: 70,
            contents_hero: None,
            cover_pattern: "images/cover_front_{tier}.png".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct CoverConfig {
    pub title: String,
    pub author: String,
    pub tier_line: String,
    pub panel_width_mm: f64,
    pub panel_offset_mm: f64,
}

impl Default for CoverConfig {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            tier_line: "{Tier} course material".to_string(),
            panel_width_mm: 170.0,
            panel_offset_mm: 20.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ContentsConfig {
    pub columns: usize,
    /// Heading of the contents page (translate it for non-English books).
    pub title: String,
}

impl Default for ContentsConfig {
    fn default() -> Self {
        Self {
            columns: 2,
            title: "Contents".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DiagramBackendConfig {
    Native,
    #[default]
    External,
    Off,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct DiagramsConfig {
    pub backend: DiagramBackendConfig,
    pub command: String,
    pub args: Vec<String>,
    pub timeout_s: u64,
    /// Path to a Mermaid config JSON. When absent the embedded default theme is used.
    pub theme: Option<String>,
    /// With `backend = "native"`: render *every* diagram type natively. By default only the
    /// types that passed the M7 gate are rendered natively and the rest go to `command`.
    pub native_only: bool,
}

impl Default for DiagramsConfig {
    fn default() -> Self {
        Self {
            backend: DiagramBackendConfig::External,
            command: "mmdc".to_string(),
            args: [
                "-i", "{input}", "-o", "{output}", "-c", "{theme}", "-e", "svg",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            timeout_s: 30,
            theme: None,
            native_only: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct BuildConfig {
    /// Relative to the output folder (plan 7.6).
    pub cache_dir: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            cache_dir: ".hideo-cache".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct DiagnosticsConfig {
    pub warnings_as_errors: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BookConfig {
    pub lang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct AccessibilityConfig {
    pub tagged: bool,
    pub pdf_ua: bool,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            tagged: true,
            pdf_ua: true,
        }
    }
}

impl Config {
    /// Parse a config from TOML text. `base_dir` is the directory relative paths resolve against.
    pub fn from_toml(text: &str, base_dir: &Utf8Path) -> Result<Self, ConfigError> {
        let mut config: Config =
            toml::from_str(text).map_err(|e| ConfigError(format!("invalid configuration: {e}")))?;
        config.base_dir = base_dir.to_owned();
        config.validate().map_err(|errors| {
            ConfigError(format!("invalid configuration:\n  {}", errors.join("\n  ")))
        })?;
        Ok(config)
    }

    /// Load the config file at `path`.
    pub fn load(path: &Utf8Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError(format!("cannot read config file {path}: {e}")))?;
        let base = path
            .parent()
            .filter(|p| !p.as_str().is_empty())
            .unwrap_or(Utf8Path::new("."));
        Self::from_toml(&text, base).map_err(|e| ConfigError(format!("{path}: {}", e.0)))
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.tiers.is_empty() {
            errors.push("tiers must not be empty".to_string());
        }
        let mut seen = HashSet::new();
        for tier in &self.tiers {
            if tier.is_empty() || tier == "." || tier == ".." {
                errors.push(format!("tier '{tier}' is not a valid folder name"));
            } else if tier.contains('/') || tier.contains('\\') {
                errors.push(format!("tier '{tier}' must not contain path separators"));
            }
            if !seen.insert(tier.as_str()) {
                errors.push(format!("duplicate tier '{tier}'"));
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
        if self.contents.columns == 0 || self.contents.columns > 4 {
            errors.push("contents.columns must be between 1 and 4".to_string());
        }
        if self.cover.title.trim().is_empty() {
            errors.push("cover.title must not be empty (it becomes the PDF title)".to_string());
        }
        if self.diagrams.backend == DiagramBackendConfig::External
            && self.diagrams.command.trim().is_empty()
        {
            errors.push(
                "diagrams.command must not be empty when diagrams.backend = \"external\""
                    .to_string(),
            );
        }
        if self.accessibility.pdf_ua && !self.accessibility.tagged {
            errors.push(
                "accessibility.pdf_ua = true requires accessibility.tagged = true".to_string(),
            );
        }
        if !is_valid_lang(&self.book.lang) {
            errors.push(format!(
                "book.lang '{}' is not a valid BCP 47 language tag (e.g. \"fi\", \"en-GB\")",
                self.book.lang
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Path of the cover image for a tier (relative to the config file).
    pub fn cover_path(&self, tier: &str) -> Utf8PathBuf {
        self.base_dir
            .join(self.images.cover_pattern.replace("{tier}", tier))
    }

    pub fn contents_hero_path(&self) -> Option<Utf8PathBuf> {
        self.images
            .contents_hero
            .as_ref()
            .map(|p| self.base_dir.join(p))
    }

    pub fn theme_path(&self) -> Option<Utf8PathBuf> {
        self.diagrams.theme.as_ref().map(|p| self.base_dir.join(p))
    }

    /// Text width in points (1 mm = 2.834645669 pt).
    pub fn text_width_pt(&self) -> f64 {
        (self.page.width_mm - 2.0 * self.page.margin_x_mm) * MM_TO_PT
    }

    pub fn tier_line(&self, tier: &str) -> String {
        self.cover
            .tier_line
            .replace("{Tier}", &capitalize(tier))
            .replace("{tier}", tier)
    }
}

pub const MM_TO_PT: f64 = 2.834_645_669;

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Two or three ASCII letters, optionally followed by subtags of 2-8 alphanumerics
/// (script or region). This is the subset Typst's `text(lang: ...)` accepts.
fn is_valid_lang(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let Some(lang) = parts.next() else {
        return false;
    };
    if !(2..=3).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let rest: Vec<&str> = parts.collect();
    if rest.len() > 2 {
        return false;
    }
    rest.iter()
        .all(|p| (2..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
tiers = ["required"]
[book]
lang = "fi"
[cover]
title = "T"
"#;

    fn load(text: &str) -> Result<Config, ConfigError> {
        Config::from_toml(text, Utf8Path::new("cfg"))
    }

    #[test]
    fn test_config_defaults() {
        let c = load(MINIMAL).unwrap();
        assert_eq!(c.page.width_mm, 210.0);
        assert_eq!(c.images.jpeg_quality, 70);
        assert_eq!(c.contents.columns, 2);
        assert!(c.accessibility.pdf_ua && c.accessibility.tagged);
        assert_eq!(c.ignore_prefixes, vec!["_", "."]);
        assert_eq!(c.build.cache_dir, ".hideo-cache");
        assert_eq!(c.diagrams.backend, DiagramBackendConfig::External);
    }

    #[test]
    fn test_config_unknown_key_errors() {
        let err = load(&format!("{MINIMAL}\n[page]\nwidht_mm = 1\n")).unwrap_err();
        assert!(err.0.contains("widht_mm"), "{}", err.0);
        let err = load(&format!("bogus = 1\n{MINIMAL}")).unwrap_err();
        assert!(err.0.contains("bogus"), "{}", err.0);
    }

    #[test]
    fn test_config_missing_book_lang_exits_2() {
        let err = load("tiers = [\"a\"]\n[cover]\ntitle = \"T\"\n").unwrap_err();
        assert!(err.0.contains("book"), "{}", err.0);
        let err = load("tiers = [\"a\"]\n[book]\n[cover]\ntitle = \"T\"\n").unwrap_err();
        assert!(err.0.contains("lang"), "{}", err.0);
    }

    #[test]
    fn test_config_invalid_book_lang_exits_2() {
        for bad in ["", "e", "english", "EN", "en_GB", "en-", "en-a"] {
            let text = MINIMAL.replace("\"fi\"", &format!("\"{bad}\""));
            let err = load(&text).unwrap_err();
            assert!(err.0.contains("book.lang"), "{bad}: {}", err.0);
        }
        for good in ["fi", "en-GB", "zh-Hans", "fil", "sr-Latn-RS"] {
            let text = MINIMAL.replace("\"fi\"", &format!("\"{good}\""));
            load(&text).unwrap_or_else(|e| panic!("{good}: {}", e.0));
        }
    }

    #[test]
    fn test_config_validation_rules() {
        assert!(load(&MINIMAL.replace("[\"required\"]", "[]")).is_err());
        assert!(load(&MINIMAL.replace("[\"required\"]", "[\"a\", \"a\"]")).is_err());
        assert!(load(&MINIMAL.replace("[\"required\"]", "[\"a/b\"]")).is_err());
        assert!(
            load(&format!(
                "{MINIMAL}\n[page]\nwidth_mm = 30\nmargin_x_mm = 20\n"
            ))
            .is_err()
        );
        assert!(load(&format!("{MINIMAL}\n[images]\njpeg_quality = 0\n")).is_err());
        assert!(load(&format!("{MINIMAL}\n[images]\ndpi = 10\n")).is_err());
        assert!(load(&format!("{MINIMAL}\n[accessibility]\ntagged = false\n")).is_err());
        assert!(
            load(&format!(
                "{MINIMAL}\n[accessibility]\ntagged = false\npdf_ua = false\n"
            ))
            .is_ok()
        );
        assert!(load(&format!("{MINIMAL}\n[diagrams]\ncommand = \"\"\n")).is_err());
        assert!(load(&MINIMAL.replace("title = \"T\"", "title = \"\"")).is_err());
    }

    #[test]
    fn test_config_relative_paths_from_config_file() {
        let text = format!(
            "{MINIMAL}\n[images]\ncontents_hero = \"img/c.png\"\n[diagrams]\ntheme = \"t.json\"\n"
        );
        let c = Config::from_toml(&text, Utf8Path::new("some/dir")).unwrap();
        assert_eq!(
            c.contents_hero_path().unwrap(),
            Utf8PathBuf::from("some/dir/img/c.png")
        );
        assert_eq!(
            c.theme_path().unwrap(),
            Utf8PathBuf::from("some/dir/t.json")
        );
        assert_eq!(
            c.cover_path("bonus"),
            Utf8PathBuf::from("some/dir/images/cover_front_bonus.png")
        );
    }

    #[test]
    fn test_tier_line_substitutes_capitalised_tier() {
        let c = load(MINIMAL).unwrap();
        assert_eq!(c.tier_line("required"), "Required course material");
    }

    #[test]
    fn example_config_in_repo_parses() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/hideo.toml")).unwrap();
        load(&text).unwrap();
    }
}
