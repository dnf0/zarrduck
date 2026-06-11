use geozarr_core::store::resolve_sync_store;
use zarrs::storage::StoreKey;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn itemcollection_resolves_to_timestack_group() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let r = resolve_sync_store(&fixt("stac_itemcollection.json"), None).expect("should resolve");
    assert_eq!(r.stac_assets.as_deref(), Some(&["band".to_string()][..]));
    let zmeta = String::from_utf8(
        r.store
            .get(&StoreKey::new(".zmetadata").unwrap())
            .unwrap()
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(zmeta.contains("band/.zarray"));
    assert!(zmeta.contains("time/.zarray"));
}

#[test]
fn empty_collection_errors() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let e = resolve_sync_store(&fixt("stac_itemcollection_empty.json"), None)
        .err()
        .expect("error");
    assert!(
        format!("{e}").to_lowercase().contains("empty") || format!("{e}").contains("no features")
    );
}

#[test]
fn missing_datetime_errors() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
    let e = resolve_sync_store(&fixt("stac_itemcollection_nodatetime.json"), None)
        .err()
        .expect("error");
    assert!(format!("{e}").to_lowercase().contains("datetime"));
}
