mod common;
use common::*;
use predicates::prelude::*;

// The ingest command uses ST_Read which expects a spatial file with a geometry
// column. Plain CSV without a geom column causes "Column geom in EXCLUDE list
// not found". We use a GeoJSON fixture instead; ST_Read produces a geom column
// (excluded) plus the lon/lat/value property columns.

#[test]
fn ingest_geojson_writes_readable_zarr() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    let out_zarr = dir.path().join("ingested.zarr");

    // Step 1: ingest GeoJSON into a Zarr store.
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("ingest")
        .arg(fixture_path("ingest_input.geojson"))
        .arg(out_zarr.to_str().unwrap())
        .args(["--value-column", "value"])
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""status": "success""#));

    // Step 2: round-trip — read the store back with `info`. The export pipeline
    // writes a single-array Zarr v3 store (zarrs 0.16) with the array at path
    // "/" inside the store root. `read_zarr_metadata` opens the same path, so
    // pointing `info` at the store root must return metadata including
    // `"array_shape"`.
    eider(&dir)
        .arg("info")
        .arg(out_zarr.to_str().unwrap())
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""array_shape""#));
}

#[test]
fn ingest_missing_input_errors() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("ingest")
        .arg("missing_input.csv")
        .arg("s3://bucket/out.zarr")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("does not exist"));
}

#[test]
fn ingest_rejects_invalid_chunks_json() {
    geozarr_ext_path();
    let dir = tempfile::tempdir().unwrap();
    // Use GeoJSON fixture so ST_Read succeeds; --chunks parse error is reached
    // only after spatial loading and ST_Read succeed.
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", dir.path())
        .arg("ingest")
        .arg(fixture_path("ingest_input.geojson"))
        .arg(dir.path().join("o.zarr").to_str().unwrap())
        .args(["--chunks", "not-json"])
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Failed to parse"));
}
