use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geozarr_core::scanner::ChunkReader;
use std::sync::Arc;
use zarrs::array::chunk_grid::RegularChunkGrid;
use zarrs::storage::store::MemoryStore;

fn bench_scanner(c: &mut Criterion) {
    // Setup a memory store and a mock array to test the scanner
    let store = Arc::new(MemoryStore::new());

    let shape = vec![100, 100, 100];
    let chunk_shape = vec![10, 10, 10];

    // We will use f32 to represent a 100x100x100 array
    let array_write = zarrs::array::ArrayBuilder::new(
        shape.clone(),
        zarrs::array::DataType::Float32,
        zarrs::array::chunk_grid::ChunkGrid::new(RegularChunkGrid::new(
            chunk_shape.clone().try_into().unwrap(),
        )),
        zarrs::array::FillValue::from(0.0f32),
    )
    .build(store.clone(), "/test")
    .unwrap();

    // Write a dummy chunk of data at [0, 0, 0]
    let chunk_data = vec![42.0f32; 1000];
    array_write.store_metadata().unwrap();
    array_write
        .store_chunk_elements(&[0, 0, 0], &chunk_data)
        .unwrap();

    let ro_store: Arc<dyn zarrs::storage::ReadableStorageTraits> = store;
    let array = zarrs::array::Array::open(ro_store, "/test").unwrap();

    let is_remote = true; // Force the remote branch
    let reader = ChunkReader::new(Arc::new(array), is_remote, shape, chunk_shape);

    let grid_coord = [0, 0, 0];
    let bounds_min = [1, 2, 3];
    let bounds_max = [8, 8, 8];

    c.bench_function("scanner_read_chunk_subset_remote", |b| {
        b.iter(|| {
            let (elements, _) = reader
                .read_chunk_subset::<f32>(
                    black_box(&grid_coord),
                    black_box(&bounds_min),
                    black_box(&bounds_max),
                )
                .unwrap();
            black_box(elements);
        });
    });
}

criterion_group!(benches, bench_scanner);
criterion_main!(benches);
