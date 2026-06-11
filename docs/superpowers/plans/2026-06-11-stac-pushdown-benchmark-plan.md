# STAC Search API Pushdown Benchmark Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone benchmark script to measure the network payload and HTTP request savings of eider's STAC Search API BBox pushdown.

**Architecture:** A lightweight in-memory HTTP server mocks a paginated STAC Search API. We test eider querying this mock server twice (with and without constraints) and compare the intercepted HTTP metrics.

**Tech Stack:** Python, `http.server` (for mocking STAC API), duckdb (running eider extension).

---

### Task 1: Create the Mock STAC Server

**Files:**
- Create: `scripts/bench_stac_pushdown.py`

- [ ] **Step 1: Write the failing server test**

```python
# scripts/bench_stac_pushdown.py
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import json
from dataclasses import dataclass, field
import urllib.parse

# Similar byte accumulator as the partial read benchmark
@dataclass
class ByteAccumulator:
    _lock: threading.Lock = field(default_factory=threading.Lock)
    _records: list = field(default_factory=list)

    def record(self, path: str, bytes_sent: int) -> None:
        with self._lock:
            self._records.append({"path": path, "bytes_sent": bytes_sent})

    def reset(self) -> None:
        with self._lock:
            self._records.clear()

    def snapshot(self) -> dict:
        with self._lock:
            records = list(self._records)
        total_bytes = sum(r["bytes_sent"] for r in records)
        return {
            "total_bytes": total_bytes,
            "n_requests": len(records),
        }

def start_stac_server() -> tuple[ThreadingHTTPServer, int, ByteAccumulator]:
    raise NotImplementedError("TODO")

class TestStacServer(unittest.TestCase):
    def test_stac_server(self):
        server, port, acc = start_stac_server()
        try:
            import urllib.request
            # Unbounded query -> many pages (returns page 1 of 100)
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/search") as res:
                data = json.loads(res.read())
                self.assertIn("features", data)
                # Next link should be present for pagination
                next_links = [l for l in data.get("links", []) if l["rel"] == "next"]
                self.assertTrue(len(next_links) > 0)
            
            acc.reset()
            # Bounded query -> 1 page
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/search?bbox=0,0,1,1") as res:
                data = json.loads(res.read())
                self.assertIn("features", data)
                # Next link should NOT be present (single page)
                next_links = [l for l in data.get("links", []) if l["rel"] == "next"]
                self.assertEqual(len(next_links), 0)
            
            snap = acc.snapshot()
            self.assertEqual(snap["n_requests"], 1)
        finally:
            server.shutdown()

if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python scripts/bench_stac_pushdown.py`
Expected: FAIL with `NotImplementedError`

- [ ] **Step 3: Write the server implementation**
Replace the NotImplementedError with the implementation:

```python
# Insert after the accumulator
def _make_stac_handler(accumulator: ByteAccumulator):
    class StacHandler(BaseHTTPRequestHandler):
        def log_message(self, *args, **kwargs) -> None:
            pass

        def do_GET(self) -> None:
            parsed = urllib.parse.urlparse(self.path)
            if parsed.path != "/search":
                self.send_error(404, "Not Found")
                return

            query = urllib.parse.parse_qs(parsed.query)
            is_bounded = "bbox" in query
            page = int(query.get("page", ["1"])[0])

            # Mock data: Unbounded = 100 pages, Bounded = 1 page
            total_pages = 1 if is_bounded else 100
            
            features = []
            # Add a mock item
            features.append({
                "type": "Feature",
                "id": f"mock_item_p{page}",
                "geometry": None,
                "bbox": [0.5, 0.5, 0.6, 0.6],
                "properties": {"datetime": "2020-01-01T00:00:00Z"},
                "assets": {
                    "data": {
                        "href": f"http://127.0.0.1:{self.server.server_address[1]}/dummy.tif",
                        "type": "image/tiff; application=geotiff; profile=cloud-optimized",
                    }
                }
            })

            links = []
            if page < total_pages:
                # Add next link
                next_url = f"http://127.0.0.1:{self.server.server_address[1]}/search?page={page+1}"
                if is_bounded:
                    next_url += f"&bbox={query['bbox'][0]}"
                links.append({"rel": "next", "href": next_url})

            response_data = {
                "type": "FeatureCollection",
                "features": features,
                "links": links
            }
            
            payload = json.dumps(response_data).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/geo+json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            accumulator.record(self.path, len(payload))

    return StacHandler

def start_stac_server() -> tuple[ThreadingHTTPServer, int, ByteAccumulator]:
    accumulator = ByteAccumulator()
    handler = _make_stac_handler(accumulator)
    
    class _QuietServer(ThreadingHTTPServer):
        daemon_threads = True
        def handle_error(self, request, client_address):
            pass

    server = _QuietServer(("127.0.0.1", 0), handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, port, accumulator
```

- [ ] **Step 4: Run test to verify it passes**

Run: `python scripts/bench_stac_pushdown.py`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add scripts/bench_stac_pushdown.py
git commit -m "bench: add mock stac server for pushdown test"
```

### Task 2: Create the eider Runners

**Files:**
- Modify: `scripts/bench_stac_pushdown.py`

- [ ] **Step 1: Write the failing tests for runners**
Append to `TestStacServer`:

```python
    def test_eider_runners(self):
        server, port, acc = start_stac_server()
        try:
            import os
            from pathlib import Path
            ext_path = Path(__file__).resolve().parents[1] / "target" / "debug" / "eider.duckdb_extension"
            if not ext_path.exists():
                self.skipTest("eider extension not built")

            bbox = (0.0, 0.0, 1.0, 1.0)
            
            # Run naive
            acc.reset()
            _, naive_bytes, naive_reqs = run_eider_stac(port, bbox, acc, ext_path, use_pushdown=False)
            self.assertEqual(naive_reqs, 100) # Should hit all 100 pages
            
            # Run pushdown
            acc.reset()
            _, push_bytes, push_reqs = run_eider_stac(port, bbox, acc, ext_path, use_pushdown=True)
            self.assertEqual(push_reqs, 1) # Should hit only 1 page
            self.assertTrue(push_bytes < naive_bytes)
        finally:
            server.shutdown()
```

- [ ] **Step 2: Run test to verify it fails**

Run: `python scripts/bench_stac_pushdown.py`
Expected: FAIL with `NameError: name 'run_eider_stac' is not defined`

- [ ] **Step 3: Write the runner implementation**

```python
# Add imports at top
from pathlib import Path
import duckdb

def _eider_conn(extension_path):
    conn = duckdb.connect(config={"allow_unsigned_extensions": True})
    conn.execute(f"LOAD '{Path(extension_path).resolve()}'")
    return conn

def run_eider_stac(
    port: int,
    bbox: tuple[float, float, float, float],
    accumulator: ByteAccumulator,
    extension_path,
    use_pushdown: bool
) -> tuple[dict, int, int]:
    """Runs eider against the STAC Search API. 
    If use_pushdown is True, uses lon_min/etc parameters.
    Otherwise, reads everything and relies on duckdb's WHERE clause."""
    
    url = f"http://127.0.0.1:{port}/search/data"
    lon_min, lat_min, lon_max, lat_max = bbox
    
    conn = _eider_conn(extension_path)
    try:
        accumulator.reset()
        if use_pushdown:
            sql = f"""
                SELECT lat, lon, value
                FROM read_geo(
                    '{url}',
                    lon_min := {lon_min}, lat_min := {lat_min},
                    lon_max := {lon_max}, lat_max := {lat_max}
                )
                -- Even with pushdown, we apply WHERE to ensure correctness 
                WHERE lon >= {lon_min} AND lon <= {lon_max}
                  AND lat >= {lat_min} AND lat <= {lat_max}
            """
        else:
            sql = f"""
                SELECT lat, lon, value
                FROM read_geo('{url}')
                WHERE lon >= {lon_min} AND lon <= {lon_max}
                  AND lat >= {lat_min} AND lat <= {lat_max}
            """
        
        # We expect this to fail gracefully because the mock server returns dummy.tif 
        # which isn't a real file, but it SHOULD make the STAC requests first.
        try:
            conn.execute(sql).fetchall()
        except duckdb.Error as e:
            pass # We only care about the STAC API requests for this benchmark
            
        snap = accumulator.snapshot()
        return {}, snap["total_bytes"], snap["n_requests"]
    finally:
        conn.close()
```

- [ ] **Step 4: Run test to verify it passes**
Run: `python scripts/bench_stac_pushdown.py test`
Expected: PASS

- [ ] **Step 5: Commit**
```bash
git add scripts/bench_stac_pushdown.py
git commit -m "bench: add eider runners for stac pushdown"
```

### Task 3: Build the Main Benchmark Harness

**Files:**
- Modify: `scripts/bench_stac_pushdown.py`

- [ ] **Step 1: Write the main harness**
Replace the `unittest.main()` block at the bottom with the timing harness:

```python
import time
import argparse
import statistics

def time_call(fn, reps=3, warmup=1):
    for _ in range(warmup):
        fn()
    samples = []
    for _ in range(reps):
        start = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - start)
    return statistics.median(samples)

def main():
    parser = argparse.ArgumentParser(description="STAC Pushdown Benchmark")
    parser.add_argument("--json", type=str, help="Output JSON path")
    parser.add_argument("--reps", type=int, default=3, help="Timing reps")
    parser.add_argument("--extension", type=str, default="target/release/eider.duckdb_extension")
    args = parser.parse_args()

    ext_path = Path(args.extension).resolve()
    if not ext_path.exists():
        print(f"Extension not found at {ext_path}. Please build it first.")
        return 1

    server, port, acc = start_stac_server()
    bbox = (0.0, 0.0, 1.0, 1.0)
    
    try:
        print(f"Benchmarking STAC Pushdown vs Naive (reps={args.reps})")
        print("-" * 60)
        print(f"{'Method':<15} | {'Requests':>10} | {'Bytes':>10} | {'Time (s)':>10}")
        print("-" * 60)

        results = {}

        # Run Naive
        def run_n():
            run_eider_stac(port, bbox, acc, ext_path, use_pushdown=False)
        
        _, naive_bytes, naive_reqs = run_eider_stac(port, bbox, acc, ext_path, use_pushdown=False)
        naive_time = time_call(run_n, reps=args.reps)
        
        print(f"{'Naive':<15} | {naive_reqs:>10} | {naive_bytes:>10} | {naive_time:>10.3f}")
        results["naive"] = {
            "requests": naive_reqs,
            "bytes": naive_bytes,
            "time_s": naive_time
        }

        # Run Pushdown
        def run_p():
            run_eider_stac(port, bbox, acc, ext_path, use_pushdown=True)
            
        _, push_bytes, push_reqs = run_eider_stac(port, bbox, acc, ext_path, use_pushdown=True)
        push_time = time_call(run_p, reps=args.reps)

        print(f"{'Pushdown':<15} | {push_reqs:>10} | {push_bytes:>10} | {push_time:>10.3f}")
        results["pushdown"] = {
            "requests": push_reqs,
            "bytes": push_bytes,
            "time_s": push_time
        }

        print("-" * 60)
        print(f"Request Reduction: {naive_reqs / max(push_reqs, 1):.1f}x")
        print(f"Byte Reduction:    {naive_bytes / max(push_bytes, 1):.1f}x")
        print(f"Speedup:           {naive_time / max(push_time, 0.0001):.1f}x")

        if args.json:
            with open(args.json, "w") as f:
                json.dump(results, f, indent=2)

    finally:
        server.shutdown()
        
    return 0

if __name__ == "__main__":
    # If run via unittest, don't run main
    import sys
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        sys.argv.pop(1)
        unittest.main()
    else:
        sys.exit(main())
```

- [ ] **Step 2: Verify it runs**
Run: `python scripts/bench_stac_pushdown.py --extension target/debug/eider.duckdb_extension`
Expected: A table comparing Naive vs Pushdown.

- [ ] **Step 3: Commit**

```bash
git add scripts/bench_stac_pushdown.py
git commit -m "bench: add cli harness for stac pushdown benchmark"
```
