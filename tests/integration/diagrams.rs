//! Diagram pipeline with a stand-in for `mmdc`: a script that writes a pre-rendered SVG.

use std::path::{Path, PathBuf};

use crate::common::*;

struct Fake {
    _dir: tempfile::TempDir,
    root: PathBuf,
    log: PathBuf,
}

/// A copy of the `diagrams` fixture whose config points at a fake renderer. The scripts run with
/// `{output}` as their first argument; `$LOG`/`%LOG%` and `$SVG`/`%SVG%` are replaced.
fn fake_project(unix: &str, windows: &str) -> Fake {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("book");
    copy_dir(&fixture("diagrams"), &root);
    let log = dir.path().join("calls.log");
    let svg = fixture("render").join("diagram.svg");
    let (command, args_toml) = if cfg!(windows) {
        let script = dir.path().join("fake-mmdc.cmd");
        let body = windows
            .replace("%LOG%", &log.to_string_lossy())
            .replace("%SVG%", &svg.to_string_lossy());
        std::fs::write(&script, format!("@echo off\r\n{body}\r\n")).unwrap();
        (
            "cmd".to_string(),
            format!("[\"/C\", '{}', \"{{output}}\"]", script.display()),
        )
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = dir.path().join("fake-mmdc.sh");
            let body = unix
                .replace("$LOG", &log.to_string_lossy())
                .replace("$SVG", &svg.to_string_lossy());
            std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            (
                script.to_string_lossy().into_owned(),
                "[\"{output}\"]".to_string(),
            )
        }
        #[cfg(not(unix))]
        {
            let _ = unix;
            unreachable!()
        }
    };
    let cfg = std::fs::read_to_string(root.join("hideo.toml")).unwrap();
    let cfg = cfg.replace(
        "backend = \"external\"",
        &format!("backend = \"external\"\ncommand = '{command}'\nargs = {args_toml}"),
    );
    std::fs::write(root.join("hideo.toml"), cfg).unwrap();
    Fake {
        _dir: dir,
        root,
        log,
    }
}

fn good_renderer() -> Fake {
    fake_project(
        "echo called >> \"$LOG\"\ncp \"$SVG\" \"$1\"",
        "echo called>> \"%LOG%\"\r\ncopy /Y \"%SVG%\" \"%~1\" >nul",
    )
}

fn calls(f: &Fake) -> usize {
    std::fs::read_to_string(&f.log)
        .map(|s| {
            s.lines()
                .filter(|l| l.trim_start().starts_with("called"))
                .count()
        })
        .unwrap_or(0)
}

fn build(root: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    hideo()
        .args(["build", "--input"])
        .arg(root)
        .arg("--output")
        .arg(out)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn test_diagrams_render_dedupe_and_embed_with_alt_text() {
    let f = good_renderer();
    let out = tempfile::tempdir().unwrap();
    let r = build(&f.root, out.path(), &["-q"]);
    assert!(r.status.success(), "{}", stderr(&r));
    // Three diagram blocks, two distinct sources: the backend ran twice.
    assert_eq!(calls(&f), 2, "identical diagrams are rendered once");

    let doc = load(&out.path().join("required.pdf"));
    let figures: Vec<String> = struct_elems(&doc)
        .iter()
        .filter(|e| e.role == "Figure")
        .map(|e| {
            String::from_utf8_lossy(e.dict.get(b"Alt").unwrap().as_str().unwrap()).into_owned()
        })
        .collect();
    assert_eq!(
        figures
            .iter()
            .filter(|a| *a == "A flow from A to B")
            .count(),
        2,
        "{figures:?}"
    );
    assert!(
        figures.contains(&"Left to right".to_string()),
        "alt from accTitle: {figures:?}"
    );
}

#[test]
fn test_diagram_cache_hit_skips_backend() {
    let f = good_renderer();
    let out = tempfile::tempdir().unwrap();
    assert!(build(&f.root, out.path(), &["-q"]).status.success());
    assert_eq!(calls(&f), 2);
    let first = std::fs::read(out.path().join("required.pdf")).unwrap();
    assert!(build(&f.root, out.path(), &["-q"]).status.success());
    assert_eq!(calls(&f), 2, "second build answered from the cache");
    assert_eq!(
        first,
        std::fs::read(out.path().join("required.pdf")).unwrap()
    );
}

#[test]
fn test_diagram_failure_names_page_and_line() {
    let f = fake_project(
        "echo boom-from-renderer >&2\nexit 1",
        "echo boom-from-renderer 1>&2\r\nexit /b 1",
    );
    let out = tempfile::tempdir().unwrap();
    let r = build(&f.root, out.path(), &["-q"]);
    assert_eq!(r.status.code(), Some(1));
    let err = stderr(&r);
    assert!(
        err.contains("error[E004]: diagram failed to render"),
        "{err}"
    );
    assert!(err.contains("page: required/01_diagrams.md"), "{err}");
    assert!(
        err.contains("line: 8"),
        "first diagram fence is on line 8 of the file: {err}"
    );
    assert!(err.contains("first line: graph TD; A-->B"), "{err}");
    assert!(err.contains("backend: external ("), "{err}");
    assert!(err.contains("boom-from-renderer"), "{err}");
    assert!(!out.path().join("required.pdf").exists());
}

#[test]
fn test_diagram_failure_stops_backend_and_suppresses_follow_on_warnings() {
    let f = fake_project(
        "echo called >> \"$LOG\"\necho boom-from-renderer >&2\nexit 1",
        "echo called>> \"%LOG%\"\r\necho boom-from-renderer 1>&2\r\nexit /b 1",
    );
    let out = tempfile::tempdir().unwrap();
    let r = build(&f.root, out.path(), &[]);

    assert_eq!(r.status.code(), Some(1));
    let err = stderr(&r);
    assert_eq!(err.matches("error[E004]").count(), 1, "{err}");
    assert_eq!(calls(&f), 1, "rendering must stop at the first failure");
    assert!(!err.contains("warning["), "{err}");
    assert!(!err.contains("Unresolved links"), "{err}");
}

#[test]
fn test_diagram_foreignobject_rejected() {
    let f = fake_project(
        "printf '<svg xmlns=\"http://www.w3.org/2000/svg\"><foreignObject/></svg>' > \"$1\"",
        "echo ^<svg xmlns=\"http://www.w3.org/2000/svg\"^>^<foreignObject/^>^</svg^> > \"%~1\"",
    );
    let out = tempfile::tempdir().unwrap();
    let r = build(&f.root, out.path(), &["-q"]);
    assert_eq!(r.status.code(), Some(1), "{}", stderr(&r));
    assert!(stderr(&r).contains("htmlLabels"), "{}", stderr(&r));
}

#[test]
fn test_diagrams_off_shows_source_as_code() {
    let f = good_renderer();
    let out = tempfile::tempdir().unwrap();
    let r = build(&f.root, out.path(), &["--diagrams", "off"]);
    assert!(r.status.success(), "{}", stderr(&r));
    assert_eq!(calls(&f), 0);
    assert!(stderr(&r).contains("diagrams are off"), "{}", stderr(&r));
    let doc = load(&out.path().join("required.pdf"));
    let text: String = (1..=page_count(&doc) as u32)
        .map(|n| page_text(&doc, n))
        .collect();
    assert!(
        text.contains("graph TD; A-->B"),
        "the Mermaid source is visible: {text}"
    );
}

#[test]
fn test_diagram_alt_from_fence_then_acctitle_then_warning() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("diagrams"), dir.path());
    std::fs::write(
        dir.path().join("required/02_no_alt.md"),
        "---\ntitle: No alt\nhero: images/hero.png\n---\n# No alt\n\n```mermaid\ngraph TD; Q-->R\n```\n",
    )
    .unwrap();
    let r = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap();
    let w008: Vec<_> = v["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["code"] == "W008")
        .collect();
    assert_eq!(
        w008.len(),
        1,
        "only the block without any alt text warns: {v}"
    );
    assert_eq!(w008[0]["page"], "required/02_no_alt.md");
    assert_eq!(w008[0]["line"], 7);
}

#[test]
fn test_check_does_not_run_the_backend() {
    let f = good_renderer();
    let r = hideo()
        .args(["check", "--input"])
        .arg(&f.root)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    assert_eq!(calls(&f), 0);
}

fn real_mmdc_available() -> bool {
    let command = if cfg!(windows) { "mmdc.cmd" } else { "mmdc" };
    std::process::Command::new(command)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn test_real_mmdc_output_passes_the_post_conditions() {
    if !real_mmdc_available() {
        assert!(
            std::env::var_os("HIDEO_REQUIRE_MMDC").is_none(),
            "mmdc is required but could not be started"
        );
        eprintln!("skipped: mmdc is not installed");
        return;
    }
    let input = tempfile::tempdir().unwrap();
    copy_dir(&fixture("diagrams"), input.path());
    let config_path = input.path().join("hideo.toml");
    let config = std::fs::read_to_string(&config_path).unwrap();
    std::fs::write(config_path, format!("{config}\ncommand = \"mmdc\"\n")).unwrap();
    // The embedded theme must keep mermaid from emitting <foreignObject> and from naming a
    // font that is not embedded (a diagnostic here names the page and line).
    let out = tempfile::tempdir().unwrap();
    let r = build(input.path(), out.path(), &[]);
    assert!(r.status.success(), "{}", stderr(&r));
    let err = stderr(&r);
    assert!(!err.contains("foreignObject"), "{err}");
    assert!(!err.contains("font-family"), "{err}");
    let doc = load(&out.path().join("required.pdf"));
    let figures = struct_elems(&doc)
        .iter()
        .filter(|e| e.role == "Figure")
        .count();
    assert!(figures >= 3, "{figures} figures");
}
