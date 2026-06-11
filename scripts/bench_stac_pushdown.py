import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import json
from dataclasses import dataclass, field
import urllib.parse
from pathlib import Path
import duckdb

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

if __name__ == "__main__":
    unittest.main()
