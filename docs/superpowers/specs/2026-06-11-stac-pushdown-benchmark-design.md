# STAC Search API Pushdown Benchmark

## Objective
Assess and demonstrate the performance benefits (in terms of network requests, bytes transferred, and wall-clock time) of the newly implemented STAC Search API Bounding Box pushdown feature in eider.

## Approach
We will create a standalone script `scripts/bench_stac_pushdown.py` to isolate and measure the STAC pagination and API routing logic. This keeps the benchmarking suite deterministic and maintainable without over-complicating the existing remote partial read benchmarks.

## Architecture

### 1. Mock STAC Server
A lightweight, in-memory Mock STAC Server will be implemented to serve a `/search` endpoint.
- **Dataset:** A synthetic "Collection" of a large number of STAC items (e.g., 10,000 items) distributed globally.
- **Behavior:**
  - **Unbounded Query:** If a client queries `/search` without a bounding box, the server returns all 10,000 items, paginated (e.g., 100 items per page). This forces the client to make 100 HTTP requests.
  - **Bounded Query:** If a client queries with a `bbox` that isolates a small region (e.g., containing only a single item), the server filters the items and returns just that 1 page.

### 2. Contenders
The benchmark will run two contenders against the exact same isolated bounding box query:
- **`eider_naive` (Control - No Pushdown):**
  Queries the dataset without passing `QueryConstraints` to the resolver. It simulates the old behavior where eider fetches all pages from the STAC API, parses them, and filters items locally.
- **`eider_pushdown` (Test - With Pushdown):**
  Queries the dataset passing `QueryConstraints`. The STAC URL builder appends `?bbox=...` to the query. The server pre-filters the results, returning only 1 page, drastically reducing local parsing and network I/O.

### 3. Metrics
A `ByteAccumulator` (similar to the one in `bench_remote_partialread.py`) will intercept HTTP traffic to record:
1.  **Total HTTP Requests:** The number of requests made to the `/search` endpoint (Expected: ~100 for naive vs 1 for pushdown).
2.  **Total Bytes Transferred:** The total JSON payload size transferred over the network.
3.  **Wall-Clock Time:** Measured as the median over 3 reps (after a warmup rep).

## Documentation
The script will output a JSON file containing the results, which can be documented alongside the existing `bench_remote_partialread.py` numbers.
