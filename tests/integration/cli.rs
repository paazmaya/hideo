//! Command line behaviour: help, exit codes, check/list formats, flags.

use crate::common::*;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    hideo()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("hideo"));
}

#[test]
fn test_cli_subcommand_help() {
    hideo()
        .args(["build", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Build PDF books"));
    hideo()
        .args(["check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Validate source"));
    hideo()
        .args(["list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List pages"));
    hideo().args(["build", "--help"]).assert().success().stdout(
        predicate::str::contains("--deny-unresolved")
            .and(predicate::str::contains("--keep-typst"))
            .and(predicate::str::contains("--jobs")),
    );
}

#[test]
fn test_exit_codes_usage_errors_are_2() {
    hideo().assert().code(2); // no subcommand
    hideo().arg("frobnicate").assert().code(2);
    hideo().args(["build", "--bogus"]).assert().code(2);
    // unknown tier
    hideo()
        .args(["check", "--input"])
        .arg(fixture("minimal"))
        .arg("nope")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown tier `nope`"));
    // missing config file
    hideo()
        .args(["check", "--input"])
        .arg(fixture("minimal").join("required"))
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot read config file"));
    // --jobs 0
    hideo()
        .args(["build", "--jobs", "0", "--input"])
        .arg(fixture("minimal"))
        .assert()
        .code(2);
}

#[test]
fn test_invalid_source_date_epoch_exits_2() {
    let out = tempfile::tempdir().unwrap();
    hideo()
        .env("SOURCE_DATE_EPOCH", "not-a-number")
        .args(["build", "--input"])
        .arg(fixture("minimal"))
        .arg("--output")
        .arg(out.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("SOURCE_DATE_EPOCH"));
    assert!(
        std::fs::read_dir(out.path()).unwrap().next().is_none(),
        "nothing was built"
    );
}

#[test]
fn test_invalid_config_exits_2_and_names_the_problem() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    std::fs::write(
        dir.path().join("hideo.toml"),
        "tiers = [\"required\"]\n[cover]\ntitle = \"T\"\n",
    )
    .unwrap();
    hideo()
        .args(["check", "--input"])
        .arg(dir.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("book"));
}

#[test]
fn test_config_flag_overrides_location() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    let cfg = dir.path().join("elsewhere.toml");
    std::fs::rename(dir.path().join("hideo.toml"), &cfg).unwrap();
    hideo()
        .args(["check", "--input"])
        .arg(dir.path())
        .assert()
        .code(2);
    hideo()
        .args(["check", "--input"])
        .arg(dir.path())
        .arg("--config")
        .arg(&cfg)
        .assert()
        .success();
}

#[test]
fn test_list_prints_pages_in_pdf_order() {
    let out = hideo()
        .args(["list", "--input"])
        .arg(fixture("full"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "required\trequired/01_intro.md\tIntroduction");
    assert_eq!(
        lines[1],
        "required\trequired/02_headings.md\tHeadings and Text"
    );
    assert!(lines.contains(&"required\trequired/sub/06_nested.md\tNested Page"));
    assert_eq!(
        lines.last().unwrap(),
        &"bonus\tbonus/01_bonus.md\tBonus Material"
    );
    // Only one tier:
    let out = hideo()
        .args(["list", "--format", "json", "--input"])
        .arg(fixture("full"))
        .arg("bonus")
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    assert_eq!(v[0]["tier"], "bonus");
    assert_eq!(v[0]["path"], "bonus/01_bonus.md");
}

#[test]
fn test_check_mode_no_pdf_written() {
    let cwd = tempfile::tempdir().unwrap();
    hideo()
        .current_dir(cwd.path())
        .args(["check", "--input"])
        .arg(fixture("full"))
        .assert()
        .success();
    assert!(
        std::fs::read_dir(cwd.path()).unwrap().next().is_none(),
        "check must not write anything"
    );
}

#[test]
fn test_check_mode_json_format() {
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(fixture("edge"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON on stdout");
    assert!(v["errors"].as_array().unwrap().is_empty(), "{v}");
    let warnings = v["warnings"].as_array().unwrap();
    let codes: Vec<&str> = warnings
        .iter()
        .map(|w| w["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"W001"), "unresolved links: {codes:?}");
    assert!(codes.contains(&"W003"), "missing hero: {codes:?}");
    for w in warnings {
        assert!(w["message"].is_string() && w.get("page").is_some() && w.get("line").is_some());
    }
    assert!(
        warnings.iter().any(|w| w["code"] == "W001"
            && w["page"] == "required/01_edge.md"
            && w["line"].is_u64())
    );
    assert_eq!(v["stats"]["tiers"], 1);
    assert_eq!(v["stats"]["pages"], 2);
}

#[test]
fn test_check_reports_content_errors_with_page_and_line_exit_1() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    std::fs::write(
        dir.path().join("required/03_bad.md"),
        "---\ntitle: Bad\nhero: images/hero.png\n---\nfine\n\nline three\n\n~~struck~~\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("required/04_noheader.md"),
        "no frontmatter at all\n",
    )
    .unwrap();
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    // Discovery errors stop the run before Markdown is looked at.
    let errors = v["errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|e| e["code"] == "E001"
            && e["page"] == "required/04_noheader.md"
            && e["line"] == 1),
        "{v}"
    );

    std::fs::remove_file(dir.path().join("required/04_noheader.md")).unwrap();
    let out = hideo()
        .args(["check", "--format", "json", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let errors = v["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1, "{v}");
    assert_eq!(errors[0]["code"], "E002");
    assert_eq!(errors[0]["page"], "required/03_bad.md");
    assert_eq!(
        errors[0]["line"], 9,
        "frontmatter is 4 lines, body line 5 is file line 9"
    );
}

#[test]
fn test_error_names_page_and_line_in_text_form() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    std::fs::write(
        dir.path().join("required/03_bad.md"),
        "---\ntitle: Bad\nhero: images/hero.png\n---\n<div>raw html</div>\n",
    )
    .unwrap();
    let out = hideo()
        .args(["check", "--input"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = stdout(&out);
    assert!(
        text.contains(
            "error[E002]: HTML blocks are not supported\n  page: required/03_bad.md\n  line: 5\n"
        ),
        "{text}"
    );
}

#[test]
fn test_deny_unresolved_exits_1() {
    let out = tempfile::tempdir().unwrap();
    let args = |deny: bool| {
        let mut a = vec![
            "build".to_string(),
            "--input".into(),
            fixture("full").to_string_lossy().into_owned(),
            "--output".into(),
            out.path().to_string_lossy().into_owned(),
        ];
        if deny {
            a.push("--deny-unresolved".into());
        }
        a
    };
    let ok = hideo().args(args(false)).output().unwrap();
    assert!(ok.status.success(), "{}", stderr(&ok));
    let denied = hideo().args(args(true)).output().unwrap();
    assert_eq!(denied.status.code(), Some(1));
    let err = stderr(&denied);
    assert!(
        err.contains("Unresolved links in required.pdf (1):"),
        "{err}"
    );
    assert!(
        err.contains("required/01_intro.md:") && err.contains("../bonus/01_bonus.md"),
        "{err}"
    );
    assert!(err.contains("outside this PDF"), "{err}");
    // PDFs are still written; the flag only changes the exit code.
    assert!(out.path().join("required.pdf").exists());
}

#[test]
fn test_quiet_hides_warnings_but_not_errors() {
    let out = tempfile::tempdir().unwrap();
    let loud = hideo()
        .args(["build", "--input"])
        .arg(fixture("full"))
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(stderr(&loud).contains("warning[W001]"));
    let quiet = hideo()
        .args(["build", "-q", "--input"])
        .arg(fixture("full"))
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(quiet.status.success());
    assert!(!stderr(&quiet).contains("warning["), "{}", stderr(&quiet));
}

#[test]
fn test_warnings_collected_and_counted() {
    let out = hideo()
        .args(["check", "--input"])
        .arg(fixture("edge"))
        .output()
        .unwrap();
    let text = stdout(&out);
    let last = text.lines().last().unwrap();
    assert!(
        last.starts_with("checked 1 tiers, 2 pages: 0 errors, "),
        "{last}"
    );
    let n: usize = last
        .rsplit(", ")
        .next()
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(text.matches("warning[").count(), n);
    assert!(n >= 5, "edge fixture should warn a lot: {n}");
}

#[test]
fn test_warnings_as_errors() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("edge"), dir.path());
    let cfg = std::fs::read_to_string(dir.path().join("hideo.toml")).unwrap();
    std::fs::write(
        dir.path().join("hideo.toml"),
        format!("{cfg}\n[diagnostics]\nwarnings_as_errors = true\n"),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let result = hideo()
        .args(["build", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{}", stderr(&result));
    assert!(stderr(&result).contains("warnings are errors"));
    assert!(!out.path().join("required.pdf").exists());
}

#[test]
fn test_pdf_ua_off_prints_warning_and_skips_validation() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture("minimal"), dir.path());
    let cfg = std::fs::read_to_string(dir.path().join("hideo.toml")).unwrap();
    std::fs::write(
        dir.path().join("hideo.toml"),
        format!("{cfg}\n[accessibility]\npdf_ua = false\n"),
    )
    .unwrap();
    let out = tempfile::tempdir().unwrap();
    let result = hideo()
        .args(["build", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(
        stderr(&result).contains("accessibility.pdf_ua is off"),
        "{}",
        stderr(&result)
    );
    // -q silences the notice
    let quiet = hideo()
        .args(["build", "-q", "--input"])
        .arg(dir.path())
        .arg("--output")
        .arg(out.path())
        .output()
        .unwrap();
    assert!(!stderr(&quiet).contains("pdf_ua is off"));
}

#[test]
fn test_native_backend_without_feature_exits_2() {
    if cfg!(feature = "native-diagrams") {
        return;
    }
    let out = tempfile::tempdir().unwrap();
    hideo()
        .args(["build", "--diagrams", "native", "--input"])
        .arg(fixture("minimal"))
        .arg("--output")
        .arg(out.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("native-diagrams"));
}

#[test]
fn test_keep_typst_writes_generated_sources() {
    let out = tempfile::tempdir().unwrap();
    let keep = tempfile::tempdir().unwrap();
    let r = hideo()
        .args(["build", "--input"])
        .arg(fixture("minimal"))
        .arg("--output")
        .arg(out.path())
        .arg("--keep-typst")
        .arg(keep.path())
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", stderr(&r));
    let main = std::fs::read_to_string(keep.path().join("required/main.typ")).unwrap();
    assert!(main.contains("#include \"pages/001.typ\""));
    assert!(keep.path().join("required/template.typ").exists());
    let page = std::fs::read_to_string(keep.path().join("required/pages/001.typ")).unwrap();
    assert!(page.contains("#heading(level: 1)[Welcome]"), "{page}");
}

#[test]
fn test_build_prints_written_pdfs_and_page_counts() {
    let (out, r) = build_fixture("minimal", &[]);
    assert!(r.status.success());
    let text = stdout(&r);
    assert!(text.contains("required.pdf (4 pages)"), "{text}");
    assert!(out.path().join("required.pdf").exists());
}

#[test]
fn test_build_single_tier_selection() {
    let (out, r) = build_fixture("full", &["bonus"]);
    assert!(r.status.success(), "{}", stderr(&r));
    assert!(out.path().join("bonus.pdf").exists());
    assert!(!out.path().join("required.pdf").exists());
    assert!(!out.path().join("additional.pdf").exists());
}
