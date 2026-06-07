use geozarr_core::store::resolve_sync_store;
use zarrs::storage::StoreKey;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn local_stac_item_resolves_to_group_with_assets() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let resolved = resolve_sync_store(&fixt("stac_item.json")).expect("STAC item should resolve");
    let zmeta = resolved
        .store
        .get(&StoreKey::new(".zmetadata").unwrap())
        .unwrap()
        .unwrap();
    let s = String::from_utf8(zmeta.to_vec()).unwrap();
    assert!(s.contains("band_uncompressed/.zarray"));
    assert!(s.contains("band_deflate/.zarray"));
}

#[test]
fn local_itemcollection_is_clear_error() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let err = resolve_sync_store(&fixt("stac_itemcollection.json"))
        .err()
        .expect("ItemCollection should error");
    let msg = format!("{err}");
    assert!(
        msg.contains("ItemCollection") || msg.contains("not yet supported"),
        "got: {msg}"
    );
}
