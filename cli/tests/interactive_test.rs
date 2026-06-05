#![cfg(unix)]

mod common;
use common::*;
use rexpect::spawn;

#[tokio::test]
async fn search_interactive_reaches_dataset_prompt() {
    let server = mock_stac().await;
    let bin = env!("CARGO_BIN_EXE_eider");
    // Provide --api so we skip the provider prompt and hit the collection list
    // served deterministically by the mock server.
    let cmd = format!("{} search --api {}", bin, server.uri());
    let mut p = spawn(&cmd, Some(10_000)).unwrap();
    // The collection is zarr-bearing, so we should reach a selection prompt or
    // (if a single result) proceed; either way no network flakiness.
    let res = p.exp_regex("(?i)Select a STAC Collection|Found .* Data URIs|Select a dataset");
    assert!(res.is_ok(), "did not reach an interactive selection prompt");
}

#[test]
fn shell_launches_or_reports_missing_duckdb() {
    if !duckdb_cli_available() {
        eprintln!("skipping shell REPL test: `duckdb` CLI not on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let db = make_extracted_db_numeric_time(&dir);
    let bin = env!("CARGO_BIN_EXE_eider");
    // TERM=dumb suppresses DuckDB's ANSI cursor-position probes (\x1b[6n)
    // which otherwise corrupt input sent over a rexpect PTY.
    let cmd = format!("env TERM=dumb {} shell {}", bin, db.display());
    let mut p = spawn(&cmd, Some(30_000)).unwrap();
    p.exp_regex("(?i)Starting DuckDB shell").unwrap();
    // DuckDB banner appears; wait for the "usage hints" line.
    p.exp_regex("(?i)usage hints").unwrap();
    // Run a query and quit the REPL.
    p.send_line("SELECT count(*) FROM extracted_data;").unwrap();
    p.exp_regex("3").unwrap();
    p.send_line(".quit").unwrap();
    p.exp_eof().unwrap();
}
