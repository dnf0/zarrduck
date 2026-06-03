# ERA Optimization Leaderboard: Coordinate Generation

**Target:** The `populate_coordinate_batch` inner loop in Zarrduck's extraction engine.
**Metric:** Execution time (lower is better).
**Baseline Command:** `cargo bench -p extension --bench coordinate_bench`

| Rank | Implementation / Branch | Bench Time | Notes / Strategy |
|------|-------------------------|------------|------------------|
| 1 | `main` (Baseline) | 173.09 µs | Initial unoptimized nested loops |
