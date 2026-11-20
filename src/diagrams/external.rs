//! External diagram backend: runs an `mmdc`-compatible command or the `gazu`
//! Pandoc filter (plan 7.7).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};
use wait_timeout::ChildExt;

use super::{DiagramError, DiagramRenderer, Theme};

pub struct ExternalRenderer {
    pub command: String,
    pub args: Vec<String>,
    pub timeout: Duration,
    gazu: bool,
}

impl ExternalRenderer {
    pub fn new(command: &str, args: &[String], timeout_s: u64) -> Self {
        let file_name = command.rsplit(['/', '\\']).next().unwrap_or(command);
        Self {
            command: command.to_string(),
            args: args.to_vec(),
            timeout: Duration::from_secs(timeout_s),
            gazu: file_name
                .strip_suffix(".exe")
                .or_else(|| file_name.strip_suffix(".cmd"))
                .unwrap_or(file_name)
                .eq_ignore_ascii_case("gazu"),
        }
    }

    /// Substitute `{input}`, `{output}` and `{theme}` per invocation.
    pub fn substitute(&self, input: &Path, output: &Path, theme: &Path) -> Vec<String> {
        self.args
            .iter()
            .map(|a| {
                a.replace("{input}", &input.to_string_lossy())
                    .replace("{output}", &output.to_string_lossy())
                    .replace("{theme}", &theme.to_string_lossy())
            })
            .collect()
    }

    fn spawn(
        &self,
        args: &[String],
        theme: &Path,
        stdout: std::fs::File,
        stderr: std::fs::File,
    ) -> std::io::Result<std::process::Child> {
        let run = |program: &str| {
            let mut command = Command::new(program);
            command
                .args(args)
                .stdout(Stdio::from(stdout.try_clone()?))
                .stderr(Stdio::from(stderr.try_clone()?));
            if self.gazu {
                command.stdin(Stdio::piped()).env("GAZU_CONFIG", theme);
            } else {
                command.stdin(Stdio::null());
            }
            command.spawn()
        };
        match run(&self.command) {
            // npm installs `mmdc.cmd` on Windows; Rust only looks for `.exe`.
            Err(e)
                if cfg!(windows)
                    && e.kind() == std::io::ErrorKind::NotFound
                    && Path::new(&self.command).extension().is_none() =>
            {
                run(&format!("{}.cmd", self.command))
            }
            other => other,
        }
    }
}

fn gazu_ast(source: &str) -> String {
    json!({
        "pandoc-api-version": [1, 23, 1],
        "meta": {},
        "blocks": [{
            "t": "CodeBlock",
            "c": [["", ["mermaid"], []], source]
        }]
    })
    .to_string()
}

fn gazu_svg(output: &str) -> Result<String, String> {
    let ast: Value =
        serde_json::from_str(output).map_err(|e| format!("invalid output AST: {e}"))?;
    let block = ast
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| {
            blocks.iter().find(|block| {
                block.get("t").and_then(Value::as_str) == Some("RawBlock")
                    && block
                        .pointer("/c/0")
                        .and_then(Value::as_str)
                        .is_some_and(|format| format.eq_ignore_ascii_case("html"))
            })
        })
        .ok_or_else(|| "the diagram was not replaced with SVG output".to_string())?;
    block
        .pointer("/c/1")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "the SVG replacement is not a string".to_string())
}

/// The last `n` non-empty lines of `text`.
fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

impl DiagramRenderer for ExternalRenderer {
    fn render(&self, source: &str, theme: &Theme) -> Result<String, DiagramError> {
        let dir = tempfile::tempdir()?;
        let input = dir.path().join("diagram.mmd");
        let output = dir.path().join("diagram.svg");
        let theme_path = dir.path().join("theme.json");
        let out_log = dir.path().join("stdout.log");
        let err_log = dir.path().join("stderr.log");
        std::fs::write(&input, source)?;
        std::fs::write(&theme_path, &theme.json)?;

        // gazu is a Pandoc filter, not an mmdc-compatible CLI. Keep accepting the
        // default mmdc-shaped args, but invoke gazu with its own HTML protocol.
        let args = if self.gazu {
            vec!["html".to_string()]
        } else {
            self.substitute(&input, &output, &theme_path)
        };
        // Output goes to files, not pipes, so a chatty child can never block on a full pipe.
        let mut child = self
            .spawn(
                &args,
                &theme_path,
                std::fs::File::create(&out_log)?,
                std::fs::File::create(&err_log)?,
            )
            .map_err(|e| {
                DiagramError::RenderFailed(format!("cannot start `{}`: {e}", self.command))
            })?;

        if self.gazu {
            child
                .stdin
                .take()
                .expect("gazu child stdin is piped")
                .write_all(gazu_ast(source).as_bytes())?;
        }

        let status = match child.wait_timeout(self.timeout)? {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(DiagramError::Timeout(self.timeout.as_secs()));
            }
        };

        if !status.success() {
            let stderr = std::fs::read_to_string(&err_log).unwrap_or_default();
            let stdout = std::fs::read_to_string(&out_log).unwrap_or_default();
            let detail = if stderr.trim().is_empty() {
                tail(&stdout, 10)
            } else {
                tail(&stderr, 10)
            };
            return Err(DiagramError::RenderFailed(format!(
                "`{}` exited with {status}\n{detail}",
                self.command
            )));
        }
        if self.gazu {
            let stdout = std::fs::read_to_string(&out_log)?;
            return gazu_svg(&stdout).map_err(|reason| {
                let stderr = std::fs::read_to_string(&err_log).unwrap_or_default();
                let detail = if stderr.trim().is_empty() {
                    reason
                } else {
                    format!("{reason}\n{}", tail(&stderr, 10))
                };
                DiagramError::RenderFailed(format!(
                    "`{}` did not render the diagram: {detail}",
                    self.command
                ))
            });
        }
        std::fs::read_to_string(&output).map_err(|e| {
            DiagramError::RenderFailed(format!("`{}` did not write the SVG: {e}", self.command))
        })
    }

    fn name(&self) -> String {
        format!("external ({})", self.command)
    }

    fn cache_id(&self) -> String {
        if self.gazu {
            format!("{}|pandoc-filter", self.name())
        } else {
            format!("{}|{}", self.name(), self.args.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script that stands in for `mmdc`, called with `{output} {input}`.
    fn script(unix: &str, windows: &str) -> (tempfile::TempDir, ExternalRenderer) {
        let dir = tempfile::tempdir().unwrap();
        let args = vec!["{output}".to_string(), "{input}".to_string()];
        let _ = unix; // only used on Unix
        if cfg!(windows) {
            let path = dir.path().join("fake.cmd");
            std::fs::write(&path, format!("@echo off\r\n{windows}\r\n")).unwrap();
            let mut all = vec!["/C".to_string(), path.to_string_lossy().into_owned()];
            all.extend(args);
            (dir, ExternalRenderer::new("cmd", &all, 20))
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let path = dir.path().join("fake.sh");
                std::fs::write(&path, format!("#!/bin/sh\n{unix}\n")).unwrap();
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
                let renderer = ExternalRenderer::new(&path.to_string_lossy(), &args, 20);
                return (dir, renderer);
            }
            #[allow(unreachable_code)]
            {
                unreachable!()
            }
        }
    }

    #[test]
    fn substitutes_placeholders() {
        let r = ExternalRenderer::new(
            "mmdc",
            &[
                "-i".into(),
                "{input}".into(),
                "-o".into(),
                "{output}".into(),
                "-c".into(),
                "{theme}".into(),
            ],
            30,
        );
        let args = r.substitute(Path::new("a.mmd"), Path::new("b.svg"), Path::new("t.json"));
        assert_eq!(args, ["-i", "a.mmd", "-o", "b.svg", "-c", "t.json"]);
    }

    #[test]
    fn detects_gazu_command_names() {
        for command in ["gazu", "gazu.exe", r"C:\tools\gazu.exe", "/tmp/gazu"] {
            let renderer = ExternalRenderer::new(command, &[], 30);
            assert!(renderer.gazu, "{command}");
        }
        assert!(!ExternalRenderer::new("mmdc", &[], 30).gazu);
    }

    #[test]
    fn extracts_svg_from_gazu_ast() {
        let output = json!({
            "blocks": [{
                "t": "RawBlock",
                "c": ["html", "<svg><text>ok</text></svg>"]
            }]
        })
        .to_string();
        assert_eq!(gazu_svg(&output).unwrap(), "<svg><text>ok</text></svg>");
    }

    #[test]
    fn gazu_unreplaced_block_reports_failure() {
        let output = json!({
            "blocks": [{
                "t": "CodeBlock",
                "c": [["", ["mermaid"], []], "not a diagram"]
            }]
        })
        .to_string();
        assert!(gazu_svg(&output).unwrap_err().contains("not replaced"));
    }

    #[test]
    fn test_diagram_external_renders() {
        let (_dir, r) = script(
            "printf '<svg>' > \"$1\"; cat \"$2\" >> \"$1\"; printf '</svg>' >> \"$1\"",
            "echo ^<svg^> > \"%~1\"\r\ntype \"%~2\" >> \"%~1\"\r\necho ^</svg^> >> \"%~1\"",
        );
        let svg = r.render("graph TD; A-->B", &Theme::embedded()).unwrap();
        assert!(
            svg.contains("<svg>") && svg.contains("graph TD; A-->B"),
            "{svg}"
        );
    }

    #[test]
    fn nonzero_exit_reports_stderr_tail() {
        let (_dir, r) = script(
            "echo oops-line-1 >&2; echo oops-line-2 >&2; exit 3",
            "echo oops-line-1 1>&2\r\necho oops-line-2 1>&2\r\nexit /b 3",
        );
        let text = r.render("x", &Theme::embedded()).unwrap_err().to_string();
        assert!(
            text.contains("oops-line-2") && text.contains("exited with"),
            "{text}"
        );
    }

    #[test]
    fn nonzero_exit_reports_stdout_tail_when_stderr_empty() {
        let (_dir, r) = script(
            "echo stdout-failure; exit 3",
            "echo stdout-failure\r\nexit /b 3",
        );
        let text = r.render("x", &Theme::embedded()).unwrap_err().to_string();
        assert!(text.contains("stdout-failure"), "{text}");
    }

    #[test]
    fn success_without_output_file_is_an_error() {
        let (_dir, r) = script("exit 0", "exit /b 0");
        let text = r.render("x", &Theme::embedded()).unwrap_err().to_string();
        assert!(text.contains("did not write"), "{text}");
    }

    #[test]
    fn missing_command_is_a_render_error() {
        let r = ExternalRenderer::new("definitely-not-a-real-command-hideo", &[], 5);
        let err = r.render("x", &Theme::embedded()).unwrap_err();
        assert!(err.to_string().contains("cannot start"), "{err}");
    }

    #[test]
    fn test_diagram_timeout_kills_child() {
        let r = if cfg!(windows) {
            ExternalRenderer::new("ping", &["-n".into(), "30".into(), "127.0.0.1".into()], 1)
        } else {
            ExternalRenderer::new("sleep", &["30".into()], 1)
        };
        let start = std::time::Instant::now();
        let err = r.render("x", &Theme::embedded()).unwrap_err();
        assert!(matches!(err, DiagramError::Timeout(1)), "{err}");
        assert!(start.elapsed() < Duration::from_secs(15));
    }

    #[test]
    fn tail_keeps_last_lines() {
        assert_eq!(tail("a\n\nb\nc\nd\n", 2), "c\nd");
        assert_eq!(tail("", 3), "");
    }
}
