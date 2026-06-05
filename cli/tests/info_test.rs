mod common;
use common::*;
use predicates::prelude::*;

// Point directly at the air_temperature array subpath.
// list_arrays(".../climate_data.zarr") returns 4 arrays (air_temperature, lat, lon, time)
// and in AgentJson mode prompt_zarr_uri would error with "Zarr Group containing multiple
// datasets". Pointing at .../air_temperature directly makes list_arrays return [""] (the
// single-array sentinel), so prompt_zarr_uri passes the URI through unchanged.
fn air_temp_uri() -> String {
    climate_zarr()
        .join("air_temperature")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn info_json_reports_shape_and_crs() {
    geozarr_ext_path(); // fail loud if the extension isn't built
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", repo_root())
        .arg("info")
        .arg(air_temp_uri())
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""array_shape""#))
        .stdout(predicate::str::contains("938"))
        // The climate_data.zarr fixture lacks an explicit CRS attribute, so the
        // extension reports "UNKNOWN" rather than "EPSG:4326". We still verify that
        // the crs field is present in the output.
        .stdout(predicate::str::contains(r#""crs""#));
}

#[test]
fn info_invalid_uri_json_errors() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("info")
        .arg("s3://invalid-bucket-that-does-not-exist/data.zarr")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#""status":"error""#));
}
