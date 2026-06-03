use criterion::{black_box, criterion_group, criterion_main, Criterion};
use zarrduck::vector_writer::populate_coordinate_batch_f64;
use geozarr_core::scanner::SubsetInfo;

fn bench_populate_coordinate(c: &mut Criterion) {
    let subset_info = SubsetInfo {
        global_starts: vec![0, 0, 0],
        shape: vec![12, 73, 144], // chunk shape from the readme
        strides: vec![73 * 144, 144, 1],
    };
    
    let batch_size = 2048; // DuckDB vector size
    let mut out_slice = vec![0.0; batch_size];
    
    // Simulate explicit coords arrays
    let lat_coords: Vec<f64> = (0..73).map(|i| 90.0 - (i as f64 * 2.5)).collect();

    c.bench_function("populate_lat_batch_2048", |b| {
        b.iter(|| {
            for cursor_offset in (0..(12*73*144)).step_by(batch_size) {
                let current_batch_size = std::cmp::min(batch_size, (12*73*144) - cursor_offset);
                populate_coordinate_batch_f64(
                    black_box(current_batch_size),
                    black_box(cursor_offset),
                    black_box(&subset_info),
                    black_box(1), // lat is dim 1
                    black_box(Some(&lat_coords)),
                    black_box(false),
                    black_box(None),
                    black_box(&mut out_slice[0..current_batch_size]),
                )
            }
        })
    });
}

criterion_group!(benches, bench_populate_coordinate);
criterion_main!(benches);
