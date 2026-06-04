use assert_cmd::Command;
use insta::assert_snapshot;

#[test]
fn test_cli_help_snapshot() {
    let mut cmd = Command::cargo_bin("eider").unwrap();
    let assert = cmd.arg("--help").assert().success();
    let output = String::from_utf8(assert.get_output().stdout.clone()).unwrap();

    // We strip out versions and trailing spaces as they can make snapshots brittle
    let cleaned_output = output
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(cleaned_output);
}
