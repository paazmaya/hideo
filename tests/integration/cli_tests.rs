use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicate::str::contains("hideo"));
}

#[test]
fn test_cli_build_help() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args(["build", "--help"]);
    cmd.assert().success().stdout(predicate::str::contains("Build PDF books"));
}

#[test]
fn test_cli_check_help() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args(["check", "--help"]);
    cmd.assert().success().stdout(predicate::str::contains("Validate source"));
}

#[test]
fn test_cli_list_help() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args(["list", "--help"]);
    cmd.assert().success().stdout(predicate::str::contains("List pages"));
}

#[test]
fn test_minimal_fixture_build() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args([
        "build",
        "--input", "tests/fixtures/minimal",
        "--output", "test-output/minimal",
    ]);
    cmd.assert().success();
}

#[test]
fn test_minimal_fixture_check() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args([
        "check",
        "--input", "tests/fixtures/minimal",
    ]);
    cmd.assert().success();
}

#[test]
fn test_minimal_fixture_list() {
    let mut cmd = Command::cargo_bin("hideo").unwrap();
    cmd.args([
        "list",
        "--input", "tests/fixtures/minimal",
    ]);
    cmd.assert().success();
}