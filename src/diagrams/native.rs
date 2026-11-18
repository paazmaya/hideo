//! Native (in-process) diagram backend, behind the `native-diagrams` feature (plan 7.7, M7).
//!
//! Uses `mermaid-rs-renderer` (pure Rust, no Node or Chromium). The same Mermaid-style JSON
//! theme as the external backend is loaded through the crate's own config loader, which only
//! reads the keys it has fields for. `render_strict` turns unsupported syntax into an error
//! instead of a silently wrong picture.
//!
//! The M7 gate result for this backend is recorded in PLAN.md.

use mermaid_rs_renderer::config::load_config;
use mermaid_rs_renderer::{RenderOptions, render_strict};

use super::{DiagramError, DiagramRenderer, Theme};

#[derive(Default)]
pub struct NativeRenderer;

impl NativeRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl DiagramRenderer for NativeRenderer {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError> {
        let dir = tempfile::tempdir()?;
        let theme_path = dir.path().join("theme.json");
        std::fs::write(&theme_path, normalise_theme(&theme.json))?;
        let config = load_config(Some(&theme_path)).map_err(|e| {
            DiagramError::RenderFailed(format!("cannot load the diagram theme: {e}"))
        })?;
        let options = RenderOptions {
            theme: config.theme,
            layout: config.layout,
        };
        render_strict(source, options).map_err(|e| DiagramError::RenderFailed(e.to_string()))
    }

    fn name(&self) -> String {
        "native (mermaid-rs-renderer)".to_string()
    }

    fn cache_id(&self) -> String {
        format!("{}|{}", self.name(), env!("CARGO_PKG_VERSION"))
    }
}

/// `mmdc` themes write sizes as CSS strings (`"22.4px"`); the native config loader wants
/// numbers. Convert every `"<number>px"` string value in `themeVariables`.
pub fn normalise_theme(json: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(json) else {
        return json.to_string();
    };
    if let Some(vars) = value
        .get_mut("themeVariables")
        .and_then(|v| v.as_object_mut())
    {
        for v in vars.values_mut() {
            let number = v
                .as_str()
                .and_then(|s| s.trim().strip_suffix("px"))
                .and_then(|n| n.trim().parse::<f64>().ok());
            if let Some(n) = number {
                *v = serde_json::json!(n);
            }
        }
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagrams::check_svg;

    /// One sample per diagram type a typical course book uses.
    pub const SAMPLES: &[(&str, &str)] = &[
        (
            "flowchart",
            "graph TD\n  A[Start] --> B{Choice}\n  B -->|yes| C[Do it]\n  B -->|no| D[Skip]\n  C --> E[End]\n  D --> E",
        ),
        (
            "flowchart-lr",
            "flowchart LR\n  A([Input]) --> B[[Process]] --> C[(Store)]",
        ),
        (
            "sequence",
            "sequenceDiagram\n  participant A as Alice\n  participant B as Bob\n  A->>B: Hello\n  B-->>A: Hi back",
        ),
        (
            "class",
            "classDiagram\n  class Animal {\n    +String name\n    +speak()\n  }\n  Animal <|-- Dog",
        ),
        (
            "state",
            "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Running: start\n  Running --> [*]: stop",
        ),
        (
            "er",
            "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  ORDER ||--|{ ITEM : contains",
        ),
        (
            "pie",
            "pie title Pets\n  \"Dogs\" : 386\n  \"Cats\" : 85\n  \"Rats\" : 15",
        ),
        (
            "gantt",
            "gantt\n  title Plan\n  dateFormat YYYY-MM-DD\n  section A\n  Task one :a1, 2026-01-01, 10d\n  Task two :after a1, 5d",
        ),
        (
            "mindmap",
            "mindmap\n  root((Book))\n    Chapters\n      One\n      Two\n    Appendix",
        ),
    ];

    #[test]
    fn px_strings_become_numbers() {
        let out: serde_json::Value = serde_json::from_str(&normalise_theme(
            r#"{"themeVariables":{"fontSize":"22.4px","fontFamily":"X","radius":8}}"#,
        ))
        .unwrap();
        assert_eq!(out["themeVariables"]["fontSize"], 22.4);
        assert_eq!(out["themeVariables"]["fontFamily"], "X");
        assert_eq!(out["themeVariables"]["radius"], 8);
        assert_eq!(normalise_theme("not json"), "not json");
    }

    #[test]
    fn native_backend_meets_the_post_conditions_for_the_flowchart_sample() {
        let svg = NativeRenderer::new()
            .render(SAMPLES[0].1, &Theme::embedded())
            .unwrap();
        let fonts = check_svg(&svg).expect("an SVG without foreignObject");
        assert!(svg.contains("<svg"));
        eprintln!("font problems: {fonts:?}");
    }

    #[test]
    fn m7_gate_report() {
        // Not an assertion about quality: it records, per diagram type, whether the native
        // backend renders and satisfies the post-conditions. The result is in PLAN.md.
        let renderer = NativeRenderer::new();
        let theme = Theme::embedded();
        for (name, source) in SAMPLES {
            let outcome = renderer
                .render(source, &theme)
                .map_err(|e| e.to_string())
                .and_then(|svg| {
                    check_svg(&svg)
                        .map(|fonts| (svg.len(), fonts))
                        .map_err(|e| e.to_string())
                });
            match outcome {
                Ok((len, fonts)) => eprintln!(
                    "M7 {name:14} OK   {len} bytes, font problems: {}",
                    fonts.len()
                ),
                Err(e) => eprintln!("M7 {name:14} FAIL {e}"),
            }
        }
    }
}
