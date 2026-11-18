use crate::diagrams::{DiagramRenderer, Theme, DiagramError};

#[cfg(feature = "native-diagrams")]
use mermaid_rs_renderer::{render_with_options, config::load_config, RenderOptions};

pub struct NativeRenderer;

impl NativeRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl DiagramRenderer for NativeRenderer {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError> {
        #[cfg(feature = "native-diagrams")]
        {
            let theme_path = std::path::Path::new("assets/mermaid-theme.json");
            let cfg = load_config(Some(theme_path))
                .map_err(|e| DiagramError::RenderFailed(format!("failed to load config: {}", e)))?;

            let options = RenderOptions {
                theme: cfg.theme,
                layout: cfg.layout,
            };

            let svg = render_with_options(source, options)
                .map_err(|e| DiagramError::RenderFailed(format!("render failed: {}", e)))?;

            Ok(svg)
        }

        #[cfg(not(feature = "native-diagrams"))]
        {
            Err(DiagramError::RenderFailed("native diagrams feature not enabled".to_string()))
        }
    }

    fn name(&self) -> &'static str {
        "native"
    }
}