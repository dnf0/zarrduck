use geozarr_core::dataset::ZarrDataset;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}
fn allow() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
}

#[test]
fn stac_asset_is_georeferenced_like_the_cog() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_item.json"), Some("band_uncompressed"), None)
        .unwrap();
    assert_eq!(ds.dim_names, vec!["lat".to_string(), "lon".to_string()]);
    assert!(ds.spatial_transform.is_some());
    let cog = ZarrDataset::open(&fixt("cog_int16_uncompressed.tif"), None).unwrap();
    assert_eq!(ds.shape, cog.shape);
    let (_, vtype) = ds.schema().unwrap().pop().unwrap();
    assert_eq!(
        format!("{vtype:?}"),
        format!("{:?}", zarrs::array::DataType::Int16)
    );
}

#[test]
fn stac_multiple_assets_without_selection_errors() {
    allow();
    let msg = match ZarrDataset::open(&fixt("stac_item.json"), None) {
        Ok(_) => panic!("expected multiple-asset selection error"),
        Err(e) => format!("{e}"),
    };
    assert!(
        msg.contains("band_uncompressed") && msg.contains("band_deflate"),
        "got: {msg}"
    );
}

#[test]
fn stac_unknown_asset_errors() {
    allow();
    let msg = match ZarrDataset::open_with_asset(&fixt("stac_item.json"), Some("nope"), None) {
        Ok(_) => panic!("expected unknown-asset error"),
        Err(e) => format!("{e}"),
    };
    assert!(msg.contains("nope") || msg.contains("Available"));
}
