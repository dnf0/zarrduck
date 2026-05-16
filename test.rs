use std::sync::Arc;
use zarrs::array::Array;
use zarrs::storage::store::FilesystemStore;

fn main() {
    let store = Arc::new(FilesystemStore::new(".").unwrap());
    let arr: Array<Arc<FilesystemStore>> = Array::open(store, "/").unwrap();
}
