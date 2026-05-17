# Parallel Scanning Optimization Design

**Date:** 2026-05-16
**Status:** Approved

## 1. Purpose & Context
Currently, the `duckdb_geozarr` extension processes chunks strictly sequentially. Because it holds a global `Mutex` across the entire `func` table function execution, DuckDB's multi-threaded worker pool is bottlenecked; all threads queue up waiting for the lock, resulting in a single thread performing all network I/O and processing. This design unlocks full parallel scanning by simulating DuckDB's `local_init` (Thread-Local State) which is currently unavailable in the `duckdb-rs` bindings.

## 2. Architecture: Simulated Thread-Local State
We will separate the state into two distinct domains: a "Global Dispatcher" and "Local Processing Buffers".

### 2.1 State Structures
- **`GlobalState`**: Tracks the progression through the Zarr chunk grid. It dictates *which* chunk is next.
  - `current_chunk_grid: Vec<u64>`
  - `exhausted: bool`
- **`LocalState`**: Tracks the processing of a specific chunk assigned to a thread.
  - `assigned_grid: Vec<u64>`
  - `local_chunk_cursor: usize`
  - `current_chunk_buffer: Option<ChunkBuffer>`

### 2.2 `InitData` Mutex Mapping
Because `InitData` is instantiated exactly once per query by DuckDB and shared across the worker pool, we can use it to hold a thread-specific map:
```rust
pub struct ReadZarrInitData {
    global_state: Mutex<GlobalState>,
    local_states: Mutex<HashMap<std::thread::ThreadId, LocalState>>,
}
```

## 3. Execution Flow (`func`)
To achieve parallelism, we must ensure no thread holds a lock during network I/O (`retrieve_chunk_elements`).

1. **Acquire Local State:** When a DuckDB worker thread enters `func`, it locks `local_states` to `take()` ownership of its `LocalState`. If none exists (first run for this thread), it initializes an empty one. It immediately unlocks `local_states`.
2. **Global Dispatch:** If the `LocalState` buffer is empty, the thread needs a new chunk. It locks `global_state`, copies `current_chunk_grid` to its local `assigned_grid`, increments the global grid for the next thread, checks for exhaustion, and unlocks `global_state`.
3. **Lock-Free I/O:** The thread performs `array.retrieve_chunk_elements(assigned_grid)`. **This happens entirely lock-free**, allowing 16 threads to download 16 different chunks simultaneously.
4. **Data Yielding:** The thread writes up to `STANDARD_VECTOR_SIZE` (2048) rows to the DuckDB `output` vector.
5. **Restore Local State:** The thread locks `local_states` and `insert()`s its `LocalState` back into the map so it can resume writing the rest of the chunk buffer on its next `func` invocation.

## 4. Safety & Trade-offs
- **Thread Collisions:** Rust guarantees `std::thread::ThreadId` is globally unique for the thread's lifetime.
- **Memory Leaks:** The `InitData` struct is tied to the query execution. Once the query completes, the `HashMap` and all lingering chunk buffers are dropped automatically.
- **Lock Contention:** The Mutex locks are only held for nanosecond-level operations (HashMap extraction and integer increments). Thread contention will be effectively zero compared to the latency of S3 object retrieval.

## 5. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The architecture maps perfectly to DuckDB's execution model and Rust's safety guarantees.
3. **Scope check:** This strictly targets the `func` loop locking mechanism.
4. **Ambiguity check:** The lock-free I/O zone is explicitly demarcated.
