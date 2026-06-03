# ERA Optimization Leaderboard: Coordinate Generation

**Target:** The `populate_coordinate_batch` inner loop in Eider's extraction engine.
**Metric:** Execution time (lower is better).
**Baseline Command:** `cargo bench -p extension --bench coordinate_bench`

| Rank | Implementation / Branch | Bench Time | Notes / Strategy |
|------|-------------------------|------------|------------------|
| 1 | `vector_writer_seed_2.rs` | ~138.68 µs | Generation 1 winner |
| 2 | `vector_writer_seed_3.rs` | ~155.72 µs | Generation 1 |
| 3 | `vector_writer_seed_5.rs` | ~156.80 µs | Generation 1 |
| 4 | `vector_writer_seed_1.rs` | ~167.53 µs | Generation 1 |
| 5 | `vector_writer_seed_4.rs` | ~169.67 µs | Generation 1 |
| 6 | `main` (Baseline) | ~170.15 µs | Initial unoptimized nested loops |
