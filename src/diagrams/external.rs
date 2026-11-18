use std::process::{Command, Stdio};
use std::io::Write;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;
use crate::diagrams::{DiagramRenderer, Theme, DiagramError};
use crate::config::DiagramsConfig;

pub struct ExternalRenderer {
    config: DiagramsConfig,
}

impl ExternalRenderer {
    pub fn new(config: DiagramsConfig) -> Self {
        Self { config }
    }

    fn substitute_args(&self, args: &[String], input_path: &std::path::Path, output_path: &std::path::Path, theme_path: &std::path::Path) -> Vec<String> {
        args.iter().map(|arg| {
            arg.replace("{input}", input_path.to_str().unwrap_or(""))
                .replace("{output}", output_path.to_str().unwrap_or(""))
                .replace("{theme}", theme_path.to_str().unwrap_or(""))
        }).collect()
    }
}

impl DiagramRenderer for ExternalRenderer {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError> {
        let mut input_file = NamedTempFile::with_suffix(".mmd")?;
        input_file.write_all(source.as_bytes())?;
        input_file.flush()?;

        let mut output_file = NamedTempFile::with_suffix(".svg")?;
        let output_path = output_file.path().to_path_buf();

        let mut theme_file = NamedTempFile::with_suffix(".json")?;
        theme_file.write_all(serde_json::to_vec_pretty(&theme.config).unwrap().as_slice())?;
        theme_file.flush()?;

        let args = self.substitute_args(&self.config.args, input_file.path(), &output_path, theme_file.path());

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| DiagramError::RenderFailed(format!("failed to spawn {}: {}", self.config.command, e)))?;

        let timeout = std::time::Duration::from_secs(self.config.timeout_s);
        let status = child.wait_timeout(timeout)
            .map_err(|_| DiagramError::Timeout)?
            .ok_or(DiagramError::Timeout)?;

        let mut stderr = String::new();
        let _ = child.stderr.take().map(|mut s| std::io::Read::read_to_string(&mut s, &mut stderr));

        if !status.success() {
            let last_lines: Vec<&str> = stderr.lines().rev().take(10).collect();
            let stderr_summary = last_lines.into_iter().rev().collect::<Vec<_>>().join("\n");
            return Err(DiagramError::RenderFailed(format!("exit code: {}, stderr:\n{}", status, stderr_summary)));
        }

        let svg = std::fs::read_to_string(&output_path)
            .map_err(|e| DiagramError::RenderFailed(format!("failed to read output: {}", e)))?;

        Ok(svg)
    }

    fn name(&self) -> &'static str {
        "external"
    }
}