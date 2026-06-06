mod common;
use common::*;
use predicates::prelude::*;

#[tokio::test]
async fn search_json_lists_zarr_uris() {
    let server = mock_stac().await;
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("search")
        .args(["--api", &server.uri()])
        .args(["--collection", "cmip6-cesm2-historical"])
        .arg("--output=json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""status":"success""#))
        // search now emits the STAC feature's self link (read_geo resolves it).
        .stdout(predicate::str::contains(
            "https://example.com/stac/items/cmip6-cesm2-item",
        ));
}

#[tokio::test]
async fn search_json_rejects_bad_bbox() {
    let server = mock_stac().await;
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("search")
        .args(["--api", &server.uri()])
        .args(["--collection", "cmip6-cesm2-historical"])
        .args(["--bbox", "1,2,3"])
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#""status":"error""#))
        .stdout(predicate::str::contains(r#""message":"#));
}

#[test]
fn search_non_interactive_requires_api() {
    let dir = tempfile::tempdir().unwrap();
    eider(&dir)
        .arg("search")
        .args(["--collection", "x"])
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains("--api is required"));
}
