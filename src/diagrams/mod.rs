pub mod native;
pub mod external;

use std::collections::HashMap;
use blake3;
use crate::config::DiagramsConfig;
use crate::error::{DiagnosticCollector, HideoError, ErrorCode, Warning, WarningCode};

pub trait DiagramRenderer: Send + Sync {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError>;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub config: serde_json::Value,
}

impl Theme {
    pub fn load(path: &Utf8PathBuf) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config = serde_json::from_str(&content)?;
        Ok(Self { config })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DiagramError {
    #[error("render failed: {0}")]
    RenderFailed(String),
    #[error("timeout")]
    Timeout,
    #[error("foreign object not allowed")]
    ForeignObject,
    #[error("invalid SVG output")]
    InvalidSvg,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct DiagramProcessor {
    config: DiagramsConfig,
    cache_dir: std::path::PathBuf,
    renderer: Box<dyn DiagramRenderer>,
    cache: HashMap<String, String>,
}

impl DiagramProcessor {
    pub fn new(config: DiagramsConfig, cache_dir: std::path::PathBuf, renderer: Box<dyn DiagramRenderer>) -> Self {
        Self {
            config,
            cache_dir,
            renderer,
            cache: HashMap::new(),
        }
    }

    pub fn render(&mut self, source: &str, page_key: &str, line: usize, diagnostics: &mut DiagnosticCollector) -> Result<String, HideoError> {
        let theme = Theme::load(&self.config.diagrams_theme_path())
            .map_err(|e| HideoError::new(ErrorCode::E004, format!("failed to load theme: {e}"))
                .with_page(page_key.to_string())
                .with_line(line))?;

        let cache_key = self.compute_cache_key(source, &theme);

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let svg = self.renderer.render(source, &theme)
            .map_err(|e| match e {
                DiagramError::ForeignObject => {
                    HideoError::new(ErrorCode::E004, "diagram contains <foreignObject>, set htmlLabels to false")
                        .with_page(page_key.to_string())
                        .with_line(line)
                }
                DiagramError::Timeout => {
                    HideoError::new(ErrorCode::E004, "diagram render timeout")
                        .with_page(page_key.to_string())
                        .with_line(line)
                }
                _ => {
                    HideoError::new(ErrorCode::E004, format!("diagram render failed: {}", e))
                        .with_page(page_key.to_string())
                        .with_line(line)
                }
            })?;

        if !svg.trim_start().starts_with("<svg") && !svg.trim_start().starts_with("<?xml") {
            return Err(HideoError::new(ErrorCode::E004, "invalid SVG output")
                .with_page(page_key.to_string())
                .with_line(line));
        }

        if svg.contains("<foreignObject") {
            return Err(HideoError::new(ErrorCode::E004, "diagram contains <foreignObject>, set htmlLabels to false")
                .with_page(page_key.to_string())
                .with_line(line));
        }

        self.cache.insert(cache_key.clone(), svg.clone());
        Ok(svg)
    }

    fn compute_cache_key(&self, source: &str, theme: &Theme) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(source.as_bytes());
        hasher.update(theme.config.to_string().as_bytes());
        hasher.update(self.renderer.name().as_bytes());
        hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
        let hash = hasher.finalize();
        format!("diagrams/{}.svg", hash.to_hex().as_str()[..16].to_string())
    }
}

pub fn create_renderer(config: &DiagramsConfig, native_diagrams: bool) -> Box<dyn DiagramRenderer> {
    match config.backend {
        crate::config::DiagramBackendConfig::Native => {
            if native_diagrams {
                Box::new(native::NativeRenderer::new())
            } else {
                Box::new(external::ExternalRenderer::new(config.clone()))
            }
        }
        crate::config::DiagramBackendConfig::External => {
            Box::new(external::ExternalRenderer::new(config.clone()))
        }
        crate::config::DiagramBackendConfig::Off => {
            Box::new(OffRenderer)
        }
    }
}

struct OffRenderer;

impl DiagramRenderer for OffRenderer {
    fn render(&self, _source: &str, _theme: &Theme) -> Result<String, DiagramError> {
        Err(DiagramError::RenderFailed("diagrams disabled".to_string()))
    }

    fn name(&self) -> &'static str {
        "off"
    }
}