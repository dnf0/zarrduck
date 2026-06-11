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

if __name__ == "__main__":
    unittest.main()
