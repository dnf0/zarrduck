mod common;
use common::*;

#[test]
fn fixtures_exist() {
    assert!(climate_zarr().exists(), "climate_data.zarr fixture missing");
    assert!(fixture_path("polygon.geojson").exists());
    assert!(fixture_path("ingest_input.csv").exists());
}

#[test]
fn synthetic_db_has_expected_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db = make_extracted_db_numeric_time(&dir);
    let conn = duckdb::Connection::open(&db).unwrap();
    let n: i64 = conn
        .query_row("SELECT count(*) FROM extracted_data", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
}

#[tokio::test]
async fn mock_stac_serves_collections() {
    let server = mock_stac().await;
    let url = format!("{}/collections", server.uri());
    let body: serde_json::Value = reqwest::get(&url).await.unwrap().json().await.unwrap();
    assert_eq!(body["collections"][0]["id"], "cmip6-cesm2-historical");
}
