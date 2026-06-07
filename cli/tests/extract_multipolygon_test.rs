mod common;
use common::*;

fn air_temp_uri() -> String {
    climate_zarr()
        .join("air_temperature")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn extract_unions_named_polygons_and_groups_per_polygon() {
    if find_geozarr_ext().is_none() {
        eprintln!("skipping: eider.duckdb_extension not built (expected on Windows)");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("multi.duckdb");

    eider(&dir)
        .env("GEOZARR_ALLOW_PATH", repo_root())
        .arg("extract")
        .arg(air_temp_uri())
        .arg(fixture_path("multi_polygon.geojson"))
        .args(["--out", out.to_str().unwrap()])
        .arg("--yes")
        .arg("--output=json")
        .assert()
        .success();

    let conn = duckdb::Connection::open(&out).unwrap();
    // One row per polygon: union extract carries each feature's `name` through.
    let mut stmt = conn
        .prepare(
            "SELECT name, COUNT(*) AS n, MAX(value)::DOUBLE AS mx \
             FROM extracted_data GROUP BY name ORDER BY name",
        )
        .unwrap();
    let rows: Vec<(String, i64, f64)> = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(
        rows.iter().map(|(n, _, _)| n.as_str()).collect::<Vec<_>>(),
        vec!["east", "west"],
        "expected exactly one row per polygon (east, west) — multi-polygon union must read every feature, got {rows:?}"
    );
    for (name, n, mx) in &rows {
        assert!(*n > 0, "polygon {name} should contain extracted cells");
        assert!(mx.is_finite(), "polygon {name} max should be finite");
    }
}
