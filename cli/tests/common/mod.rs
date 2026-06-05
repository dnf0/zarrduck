//! Shared test harness for eider CLI integration tests.
//!
//! Each integration test file does `mod common;` and `use common::*;`.
#![allow(dead_code)] // not every test file uses every helper

use std::path::PathBuf;

use assert_cmd::Command;
use duckdb::Connection;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Absolute path to the CLI crate root (`cli/`).
pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Absolute path to the workspace root (parent of `cli/`).
pub fn repo_root() -> PathBuf {
    manifest_dir().parent().unwrap().to_path_buf()
}

/// Path to a file under `cli/tests/fixtures/`.
pub fn fixture_path(name: &str) -> PathBuf {
    manifest_dir().join("tests").join("fixtures").join(name)
}

/// Path to the real Zarr fixture at the repo root.
pub fn climate_zarr() -> PathBuf {
    repo_root().join("climate_data.zarr")
}

/// Locate the built eider DuckDB extension, or fail loudly with guidance.
pub fn geozarr_ext_path() -> PathBuf {
    let candidates = [
        repo_root().join("target/debug/eider.duckdb_extension"),
        repo_root().join("target/release/eider.duckdb_extension"),
    ];
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| {
            panic!(
                "eider.duckdb_extension not found in target/{{debug,release}}. \
                 Build it first with: cargo duckdb-ext build"
            )
        })
}

/// True when the external `duckdb` CLI is on PATH (gates shell REPL tests).
pub fn duckdb_cli_available() -> bool {
    std::process::Command::new("duckdb")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A configured `assert_cmd::Command` for the `eider` binary, isolated in `dir`.
pub fn eider(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("eider").unwrap();
    cmd.current_dir(dir.path());
    // Cache DuckDB-managed extensions (e.g. spatial) in a stable per-user dir so
    // the network fetch happens at most once across the whole suite.
    cmd.env(
        "DUCKDB_EXTENSION_DIRECTORY",
        repo_root().join(".duckdb_ext_cache"),
    );
    cmd
}

/// Build a deterministic `.duckdb` file containing an `extracted_data` table with
/// a numeric (unix-seconds) time column. Returns the path to the db file.
///
/// Rows (lat=10, lon=20):
///   2020-01-15 -> 2.0, 2020-06-15 -> 4.0, 2021-01-15 -> 10.0
/// so year-avg gives 2020 => 3.0, 2021 => 10.0.
pub fn make_extracted_db_numeric_time(dir: &TempDir) -> PathBuf {
    let db_path = dir.path().join("extracted.duckdb");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE extracted_data (
             time BIGINT,
             lat DOUBLE,
             lon DOUBLE,
             air_temperature DOUBLE
         );
         INSERT INTO extracted_data VALUES
             (1579046400, 10.0, 20.0, 2.0),   -- 2020-01-15
             (1592179200, 10.0, 20.0, 4.0),   -- 2020-06-15
             (1610668800, 10.0, 20.0, 10.0);  -- 2021-01-15",
    )
    .unwrap();
    db_path
}

/// Build a deterministic `.duckdb` file with a 3-timestamp × 2-lat × 2-lon grid
/// (12 rows total) suitable for non-degenerate heatmap and line/hist plots.
///
/// Grid:
///   lat ∈ {10.0, 20.0}, lon ∈ {30.0, 40.0}
///   times: 2020-01-15, 2020-06-15, 2021-01-15
///   air_temperature varies across all cells (2.0 – 13.0)
pub fn make_plot_db(dir: &TempDir) -> PathBuf {
    let db_path = dir.path().join("plot.duckdb");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE extracted_data (
             time BIGINT,
             lat DOUBLE,
             lon DOUBLE,
             air_temperature DOUBLE
         );
         INSERT INTO extracted_data VALUES
             (1579046400, 10.0, 30.0,  2.0),
             (1579046400, 10.0, 40.0,  4.0),
             (1579046400, 20.0, 30.0,  6.0),
             (1579046400, 20.0, 40.0,  8.0),
             (1592179200, 10.0, 30.0,  3.0),
             (1592179200, 10.0, 40.0,  5.0),
             (1592179200, 20.0, 30.0,  7.0),
             (1592179200, 20.0, 40.0,  9.0),
             (1610668800, 10.0, 30.0, 10.0),
             (1610668800, 10.0, 40.0, 11.0),
             (1610668800, 20.0, 30.0, 12.0),
             (1610668800, 20.0, 40.0, 13.0);",
    )
    .unwrap();
    db_path
}

/// Start an in-process mock STAC server. Mirrors scripts/mock_stac.py:
/// GET /collections lists one zarr-bearing collection; POST /search returns one
/// feature with a zarr asset. Returns the running server (keep it alive for the
/// duration of the test; its `.uri()` is the `--api` base URL).
pub async fn mock_stac() -> MockServer {
    let server = MockServer::start().await;

    let collections_body = serde_json::json!({
        "collections": [{
            "id": "cmip6-cesm2-historical",
            "title": "CMIP6 CESM2 Historical Surface Temperature",
            "description": "Near-surface air temperature, monthly means.",
            "assets": {
                "data": {
                    "href": "https://example.com/cmip6/tas.zarr",
                    "type": "application/vnd+zarr"
                }
            }
        }]
    });

    let search_body = serde_json::json!({
        "features": [{
            "assets": {
                "data": {
                    "href": "https://example.com/cmip6/tas.zarr",
                    "type": "application/vnd+zarr",
                    "title": "CMIP6 CESM2 Near-Surface Air Temperature",
                    "description": "Monthly mean near-surface air temperature (tas)."
                }
            }
        }]
    });

    Mock::given(method("GET"))
        .and(path("/collections"))
        .respond_with(ResponseTemplate::new(200).set_body_json(collections_body))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(search_body))
        .mount(&server)
        .await;

    server
}
