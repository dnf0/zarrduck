use std::sync::Arc;

fn main() {
    let path = "climate_data.zarr/air_temperature";
    let canon = std::fs::canonicalize(path).unwrap();
    println!("Canon: {:?}", canon);

    let store = zarrs::storage::store::FilesystemStore::new(path).unwrap();
    let store = Arc::new(store);

    match zarrs::array::Array::open(store.clone(), "/") {
        Ok(arr) => println!("Success! {:?}", arr.shape()),
        Err(e) => println!("Error: {}", e),
    }
}
