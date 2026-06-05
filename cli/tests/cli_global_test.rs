mod common;
use common::*;
use predicates::prelude::*;

#[test]
fn help_describes_the_tool() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Agentic Spatial Data Engine"));
}

#[test]
fn completions_generate_for_all_shells() {
    for shell in ["bash", "zsh", "fish", "powershell"] {
        let dir = tempfile::tempdir().unwrap();
        eider(&dir)
            .arg("completions")
            .arg(shell)
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn json_error_envelope_shape_on_missing_input() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("resample")
        .arg("does_not_exist.duckdb")
        .arg("out.duckdb")
        .args(["--freq", "year", "--agg", "avg", "--output=json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#""status":"error""#))
        .stdout(predicate::str::contains(r#""message":"#));
}
