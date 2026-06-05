use geozarr_core::store::resolve_sync_store;
use zarrs::storage::ListableStorageTraits;
use zarrs::storage::ReadableStorageTraits;

#[test]
fn test_stac_fallback() {
    let path = "https://earth-search.aws.element84.com/v1/collections/sentinel-2-pre-c1-l2a/items/S2B_T21NYC_20221205T140704_L2A/swir22";
    println!("Testing resolve_sync_store with path: {}", path);
    match resolve_sync_store(path) {
        Ok(resolved) => {
            let key = zarrs::storage::StoreKey::new(".zarray").unwrap();
            let data = resolved.store.get(&key).unwrap();
            if let Some(bytes) = data {
                let json = String::from_utf8(bytes.to_vec()).unwrap();
                println!("SUCCESS! Array metadata:\n{}", json);
                assert!(json.contains("zarr_format"));
            } else {
                panic!("No .zarray found! Store returned Ok(None).");
            }
        }
        Err(e) => {
            panic!("ERROR: {}", e);
        }
    }
}
