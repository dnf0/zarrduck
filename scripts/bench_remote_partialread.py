"""Remote partial-read benchmark foundation (Task 1).

Provides:
  * A Range-capable, byte-logging HTTP server (``start_server``) that serves
    files from a root directory, supports HTTP ``Range`` requests (returns
    ``206 Partial Content`` with a correct ``Content-Range`` header and the
    requested byte slice), and records per-request byte accounting into a
    thread-safe accumulator.
  * Data generators for a lat/lon-chunked Zarr v2 store and a tiled
    EPSG:4326 COG, plus a centered-window bbox helper.

The server is the substrate for the partial-read benchmark: a Zarr chunk is a
single full-file GET, while a COG window read issues ``Range`` GETs, so the
server MUST support Range to measure bytes fetched honestly.
"""

from __future__ import annotations

import os
import re
import threading
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Optional

import numpy as np

# --- Constants -------------------------------------------------------------

# Synthetic grid spans a small EPSG:4326 window so coordinates are plausible
# degrees. Centered on the prime meridian / equator for symmetry.
LON_MIN_DEG = -10.0
LON_MAX_DEG = 10.0
LAT_MIN_DEG = -10.0
LAT_MAX_DEG = 10.0

ZARR_VAR_NAME = "air_temperature"


# --- Byte-logging accumulator ---------------------------------------------


@dataclass
class _RequestRecord:
    path: str
    bytes_sent: int
    is_range: bool


@dataclass
class ByteAccumulator:
    """Thread-safe record of bytes served per HTTP request.

    Each handled GET appends one record. ``snapshot`` returns an aggregate that
    callers compare before/after a read to attribute bytes/requests to a
    contender.
    """

    _lock: threading.Lock = field(default_factory=threading.Lock)
    _records: list[_RequestRecord] = field(default_factory=list)

    def record(self, path: str, bytes_sent: int, is_range: bool) -> None:
        with self._lock:
            self._records.append(
                _RequestRecord(path=path, bytes_sent=bytes_sent, is_range=is_range)
            )

    def reset(self) -> None:
        with self._lock:
            self._records.clear()

    def snapshot(self) -> dict:
        """Return aggregate stats: total_bytes, n_requests, per-path counts."""
        with self._lock:
            records = list(self._records)
        per_path: dict[str, dict[str, int]] = {}
        total_bytes = 0
        for r in records:
            total_bytes += r.bytes_sent
            entry = per_path.setdefault(r.path, {"bytes": 0, "requests": 0})
            entry["bytes"] += r.bytes_sent
            entry["requests"] += 1
        return {
            "total_bytes": total_bytes,
            "n_requests": len(records),
            "per_path": per_path,
        }


# --- Range-capable HTTP handler --------------------------------------------

# Matches a single closed/open byte range, e.g. "bytes=0-99" or "bytes=100-".
# Multi-range requests are not used by the benchmark contenders, so we only
# honor the first range spec.
_RANGE_RE = re.compile(r"^bytes=(\d*)-(\d*)$")


def _make_handler(root_dir: Path, accumulator: ByteAccumulator):
    """Build a request-handler class bound to a root dir + accumulator."""

    root = root_dir.resolve()

    class RangeLoggingHandler(BaseHTTPRequestHandler):
        # Silence the default stderr request logging to keep bench output clean.
        def log_message(self, *args, **kwargs) -> None:  # noqa: D401
            pass

        def _resolve(self) -> Optional[Path]:
            """Map the URL path to a file inside root, guarding traversal."""
            rel = self.path.lstrip("/").split("?", 1)[0]
            target = (root / rel).resolve()
            if root != target and root not in target.parents:
                return None
            if not target.is_file():
                return None
            return target

        def _parse_range(self, size: int) -> Optional[tuple[int, int]]:
            """Parse the Range header into an inclusive (start, end) pair."""
            header = self.headers.get("Range")
            if not header:
                return None
            m = _RANGE_RE.match(header.strip())
            if not m:
                return None
            start_s, end_s = m.group(1), m.group(2)
            if start_s == "" and end_s == "":
                return None
            if start_s == "":
                # Suffix range: last N bytes.
                length = int(end_s)
                if length == 0:
                    return None
                start = max(0, size - length)
                end = size - 1
            else:
                start = int(start_s)
                end = int(end_s) if end_s != "" else size - 1
            end = min(end, size - 1)
            if start > end or start >= size:
                return None
            return start, end

        def do_GET(self) -> None:  # noqa: N802 (http.server API)
            target = self._resolve()
            if target is None:
                self.send_error(404, "Not Found")
                return
            size = target.stat().st_size
            rng = self._parse_range(size)
            data = target.read_bytes()

            if rng is not None:
                start, end = rng
                chunk = data[start : end + 1]
                self.send_response(206)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
                self.send_header("Content-Length", str(len(chunk)))
                self.send_header("Accept-Ranges", "bytes")
                self.end_headers()
                self.wfile.write(chunk)
                accumulator.record(self.path, len(chunk), is_range=True)
            else:
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Length", str(size))
                self.send_header("Accept-Ranges", "bytes")
                self.end_headers()
                self.wfile.write(data)
                accumulator.record(self.path, size, is_range=False)

        def do_HEAD(self) -> None:  # noqa: N802
            target = self._resolve()
            if target is None:
                self.send_error(404, "Not Found")
                return
            size = target.stat().st_size
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(size))
            self.send_header("Accept-Ranges", "bytes")
            self.end_headers()

    return RangeLoggingHandler


def start_server(root_dir) -> tuple[ThreadingHTTPServer, int, ByteAccumulator]:
    """Start a Range-capable byte-logging server on an ephemeral port.

    Returns ``(server, port, accumulator)``. The server runs in a daemon
    thread; call ``server.shutdown()`` to stop it.
    """
    root = Path(root_dir)
    accumulator = ByteAccumulator()
    handler = _make_handler(root, accumulator)
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, port, accumulator


# --- Generators ------------------------------------------------------------


def generate_zarr(
    out_dir,
    shape: tuple[int, int] = (4000, 4000),
    chunks: tuple[int, int] = (256, 256),
    seed: int = 42,
) -> dict:
    """Generate a 2D float32 Zarr v2 store with lat/lon coordinate arrays.

    Values are a smooth gradient plus reproducible noise. Written via
    ``xarray.to_zarr`` as **zarr v2 with consolidated metadata** so the eider
    extension (Zarr v2 reader) and ``xarray.open_zarr`` both work, and ``.sel``
    on lat/lon resolves windows.

    Returns the store path and coordinate info.
    """
    import xarray as xr

    out_dir = Path(out_dir)
    height, width = shape
    rng = np.random.default_rng(seed)

    # Monotonic ascending lat/lon in EPSG:4326-ish degrees (cell centers).
    lat = np.linspace(LAT_MIN_DEG, LAT_MAX_DEG, height, dtype="float64")
    lon = np.linspace(LON_MIN_DEG, LON_MAX_DEG, width, dtype="float64")

    yy, xx = np.meshgrid(
        np.linspace(0.0, 1.0, height), np.linspace(0.0, 1.0, width), indexing="ij"
    )
    gradient = (yy + xx).astype("float32")
    noise = rng.standard_normal(size=shape).astype("float32") * 0.01
    values = (gradient + noise).astype("float32")

    da = xr.DataArray(
        values,
        dims=("lat", "lon"),
        coords={"lat": lat, "lon": lon},
        name=ZARR_VAR_NAME,
    )
    ds = da.to_dataset()

    store_path = out_dir / "store.zarr"
    # Set chunk sizes via zarr encoding (no dask dependency). zarr_format=2
    # keeps v2 metadata (.zarray); consolidated writes .zmetadata.
    ds.to_zarr(
        str(store_path),
        mode="w",
        consolidated=True,
        zarr_format=2,
        encoding={ZARR_VAR_NAME: {"chunks": chunks}},
    )

    return {
        "store": store_path,
        "var": ZARR_VAR_NAME,
        "shape": shape,
        "chunks": chunks,
        "lat": lat,
        "lon": lon,
        "lat_min": float(lat[0]),
        "lat_max": float(lat[-1]),
        "lon_min": float(lon[0]),
        "lon_max": float(lon[-1]),
    }


def generate_cog(
    path,
    shape: tuple[int, int] = (4000, 4000),
    blocksize: int = 256,
    seed: int = 42,
) -> dict:
    """Generate a tiled, north-up EPSG:4326 single-band float32 GeoTIFF.

    EPSG:4326 + north-up is required so eider's COG bbox pushdown (which binds
    only for 4326) works. Values mirror ``generate_zarr`` (gradient + noise).

    Returns the path and transform/coord info.
    """
    import rasterio
    from rasterio.transform import from_bounds

    path = Path(path)
    height, width = shape
    rng = np.random.default_rng(seed)

    yy, xx = np.meshgrid(
        np.linspace(0.0, 1.0, height), np.linspace(0.0, 1.0, width), indexing="ij"
    )
    gradient = (yy + xx).astype("float32")
    noise = rng.standard_normal(size=shape).astype("float32") * 0.01
    values = (gradient + noise).astype("float32")

    # North-up transform: row 0 is the northern (max-lat) edge.
    transform = from_bounds(
        LON_MIN_DEG, LAT_MIN_DEG, LON_MAX_DEG, LAT_MAX_DEG, width, height
    )

    profile = {
        "driver": "GTiff",
        "dtype": "float32",
        "count": 1,
        "height": height,
        "width": width,
        "crs": "EPSG:4326",
        "transform": transform,
        "tiled": True,
        "blockxsize": blocksize,
        "blockysize": blocksize,
        "compress": "deflate",
    }
    with rasterio.open(str(path), "w", **profile) as dst:
        dst.write(values, 1)

    return {
        "path": path,
        "shape": shape,
        "blocksize": blocksize,
        "transform": transform,
        "crs": "EPSG:4326",
        "lon_min": LON_MIN_DEG,
        "lon_max": LON_MAX_DEG,
        "lat_min": LAT_MIN_DEG,
        "lat_max": LAT_MAX_DEG,
    }


def window_bbox(coords_or_transform, fraction: float) -> tuple[float, float, float, float]:
    """Return a centered bbox covering ~``fraction`` of the grid area.

    Accepts either a generator info dict (with lon_min/lon_max/lat_min/lat_max)
    or a transform-bearing object; the bbox side length is
    ``sqrt(fraction)`` of each axis so the covered area is ``fraction`` of total.

    Returns ``(lon_min, lat_min, lon_max, lat_max)``.
    """
    if not (0.0 < fraction <= 1.0):
        raise ValueError(f"fraction must be in (0, 1], got {fraction}")

    if isinstance(coords_or_transform, dict):
        lon_min = coords_or_transform["lon_min"]
        lon_max = coords_or_transform["lon_max"]
        lat_min = coords_or_transform["lat_min"]
        lat_max = coords_or_transform["lat_max"]
    else:
        raise TypeError(
            "window_bbox expects a generator info dict with lon/lat bounds"
        )

    lon_span = lon_max - lon_min
    lat_span = lat_max - lat_min
    lon_center = (lon_min + lon_max) / 2.0
    lat_center = (lat_min + lat_max) / 2.0

    side = float(np.sqrt(fraction))
    half_lon = lon_span * side / 2.0
    half_lat = lat_span * side / 2.0

    return (
        lon_center - half_lon,
        lat_center - half_lat,
        lon_center + half_lon,
        lat_center + half_lat,
    )


# --- Contender runners (Task 2) -------------------------------------------
#
# Each runner reads the SAME query window over HTTP and returns
# ``(summary, bytes_fetched, n_requests)`` where ``summary`` is a deterministic
# value fingerprint of the window's cells:
#   {count, sum, max, min}
# ``sum`` is rounded so float-accumulation order differences between numpy and
# DuckDB do not trip the correctness gate. bytes/requests come from the shared
# server accumulator (reset before the read, snapshot after).
#
# Window alignment (CRITICAL — see plan): all contenders must score the SAME
# set of cells so ``count`` matches EXACTLY.
#   * The Zarr coordinate arrays are ascending lat/lon (cell centres). Both
#     eider's bbox pushdown and xarray ``.sel(slice(min, max))`` are INCLUSIVE
#     of cell centres in ``[min, max]``, so they pick the identical cells.
#   * eider's COG bbox pushdown returns every cell of each *intersecting tile*
#     (tile-granular), so it can include a fringe of cells just outside the
#     bbox. We therefore post-filter eider's returned cells to the closed bbox
#     ``[lon_min, lon_max] x [lat_min, lat_max]`` — exactly the cells the
#     chunk-aware ``.sel`` / ``clip_box`` selects — before summarising.
#   * The COG is north-up (descending row -> lat), but because we summarise by
#     cell-centre coordinate (not array index) the lat ordering does not affect
#     which cells fall in the window.

# Eider's COG bbox pushdown only binds for EPSG:4326 single-Feature STAC items
# whose asset URL is addressed as ``<item-url>/<asset-name>`` (the direct
# ``.tif`` HTTP form does not split endpoint/key correctly in the current
# extension; see the run report). These constants name the STAC scaffold the
# eider COG runner serves alongside the raw COG.
COG_STAC_ITEM_REL = "stac/item.json"
COG_ASSET_NAME = "data"

# GDAL/vsicurl tuning so ``/vsicurl`` issues HEAD + ranged GETs against the
# Range-capable server and does NOT cache between reads (so the accumulator
# attributes bytes honestly per call).
_GDAL_VSICURL_ENV = {
    "CPL_VSIL_CURL_ALLOWED_EXTENSIONS": ".tif",
    "GDAL_DISABLE_READDIR_ON_OPEN": "EMPTY_DIR",
    "VSI_CACHE": "FALSE",
    "CPL_VSIL_CURL_CACHE_SIZE": "0",
    "GDAL_HTTP_MULTIRANGE": "YES",
}

# Decimal places the window sum is rounded to for the correctness fingerprint.
_SUMMARY_SUM_DECIMALS = 3


def _set_gdal_vsicurl_env() -> None:
    """Set GDAL ``/vsicurl`` env in-process (idempotent)."""
    for key, value in _GDAL_VSICURL_ENV.items():
        os.environ[key] = value


def _summary(values: np.ndarray) -> dict:
    """Deterministic value fingerprint of a window's finite cells."""
    finite = values[np.isfinite(values)]
    count = int(finite.size)
    if count == 0:
        return {"count": 0, "sum": 0.0, "max": float("nan"), "min": float("nan")}
    return {
        "count": count,
        "sum": round(float(finite.sum(dtype="float64")), _SUMMARY_SUM_DECIMALS),
        "max": float(finite.max()),
        "min": float(finite.min()),
    }


def _eider_conn(extension_path):
    """Open a DuckDB connection with the (unsigned) eider extension loaded."""
    import duckdb

    conn = duckdb.connect(config={"allow_unsigned_extensions": True})
    conn.execute(f"LOAD '{Path(extension_path).resolve()}'")
    return conn


def write_cog_stac_item(root_dir, port: int, cog_rel: str = "grid.tif") -> str:
    """Write a minimal EPSG:4326 STAC Item beside the COG and return its rel URL.

    eider's COG bbox pushdown binds through the single-Feature STAC item path;
    the returned value is the path (relative to the server root) to address as
    ``http://127.0.0.1:{port}/{rel}/{COG_ASSET_NAME}`` in ``read_geo``.
    """
    import json

    root = Path(root_dir)
    item_path = root / COG_STAC_ITEM_REL
    item_path.parent.mkdir(parents=True, exist_ok=True)
    item = {
        "stac_version": "1.0.0",
        "type": "Feature",
        "id": "remote-bench-cog",
        "geometry": None,
        "bbox": [LON_MIN_DEG, LAT_MIN_DEG, LON_MAX_DEG, LAT_MAX_DEG],
        "properties": {"datetime": "2020-01-01T00:00:00Z"},
        "assets": {
            COG_ASSET_NAME: {
                "href": f"http://127.0.0.1:{port}/{cog_rel}",
                "type": "image/tiff; application=geotiff; profile=cloud-optimized",
            }
        },
    }
    item_path.write_text(json.dumps(item), encoding="utf-8")
    return COG_STAC_ITEM_REL


def run_eider_remote(
    port: int,
    store_path: str,
    bbox: tuple[float, float, float, float],
    accumulator: ByteAccumulator,
    extension_path,
    *,
    fmt: str,
) -> tuple[dict, int, int]:
    """eider runner: ``read_geo`` over HTTP with lon/lat bbox pushdown.

    ``store_path`` is the server-relative path:
      * Zarr -> ``store.zarr/<var>``
      * COG  -> ``stac/item.json/data`` (STAC-item asset addressing; the bare
        ``.tif`` HTTP form is not yet readable by the extension).
    ``bbox`` is ``(lon_min, lat_min, lon_max, lat_max)``. Returns the window
    summary plus bytes/requests fetched during the read.
    """
    lon_min, lat_min, lon_max, lat_max = bbox
    url = f"http://127.0.0.1:{port}/{store_path}"
    conn = _eider_conn(extension_path)
    try:
        accumulator.reset()
        cols = conn.execute(
            f"""
            SELECT lat, lon, value
            FROM read_geo(
                '{url}',
                lon_min := {lon_min}, lat_min := {lat_min},
                lon_max := {lon_max}, lat_max := {lat_max}
            )
            """
        ).fetchnumpy()
        snap = accumulator.snapshot()
    finally:
        conn.close()

    lat = np.asarray(cols["lat"], dtype="float64")
    lon = np.asarray(cols["lon"], dtype="float64")
    value = np.asarray(cols["value"], dtype="float64")
    if fmt == "cog":
        # COG pushdown is tile-granular: clip to the closed bbox so the cell set
        # matches the chunk-aware/naive ``.sel`` window exactly.
        mask = (
            (lon >= lon_min)
            & (lon <= lon_max)
            & (lat >= lat_min)
            & (lat <= lat_max)
        )
        value = value[mask]
    return _summary(value), snap["total_bytes"], snap["n_requests"]


def run_chunkaware_remote(
    port: int,
    bbox: tuple[float, float, float, float],
    accumulator: ByteAccumulator,
    *,
    fmt: str,
    cog_rel: str = "grid.tif",
) -> tuple[dict, int, int]:
    """Chunk-aware baseline: fetch only the chunks/tiles intersecting the window.

    Zarr -> ``xarray.open_zarr(fsspec http mapper)`` + ``.sel`` (lat ascending).
    COG  -> ``rioxarray.open_rasterio('/vsicurl/...')`` + ``clip_box`` (windowed
    ranged reads). Both fetch a strict subset, not the whole store.
    """
    lon_min, lat_min, lon_max, lat_max = bbox
    if fmt == "zarr":
        import fsspec
        import xarray as xr

        accumulator.reset()
        mapper = fsspec.get_mapper(f"http://127.0.0.1:{port}/store.zarr")
        ds = xr.open_zarr(mapper, consolidated=True)
        # Zarr lat/lon are ascending cell centres -> slice(min, max) (inclusive).
        sub = ds[ZARR_VAR_NAME].sel(
            lat=slice(lat_min, lat_max), lon=slice(lon_min, lon_max)
        ).load()
        snap = accumulator.snapshot()
        return _summary(sub.values), snap["total_bytes"], snap["n_requests"]

    if fmt == "cog":
        import rioxarray  # noqa: F401  (registers the .rio accessor)

        _set_gdal_vsicurl_env()
        accumulator.reset()
        url = f"/vsicurl/http://127.0.0.1:{port}/{cog_rel}"
        da = rioxarray.open_rasterio(url)
        clipped = da.rio.clip_box(
            minx=lon_min, miny=lat_min, maxx=lon_max, maxy=lat_max
        ).load()
        snap = accumulator.snapshot()
        return _summary(clipped.values), snap["total_bytes"], snap["n_requests"]

    raise ValueError(f"unknown fmt {fmt!r}")


def run_naive_remote(
    port: int,
    bbox: tuple[float, float, float, float],
    accumulator: ByteAccumulator,
    *,
    fmt: str,
    cog_rel: str = "grid.tif",
) -> tuple[dict, int, int]:
    """Naive baseline: read the WHOLE array, then subset in memory.

    Fetches ~the entire store; the in-memory subset uses the same closed-bbox
    cell selection as the other contenders so the summaries are comparable.
    """
    lon_min, lat_min, lon_max, lat_max = bbox
    if fmt == "zarr":
        import fsspec
        import xarray as xr

        accumulator.reset()
        mapper = fsspec.get_mapper(f"http://127.0.0.1:{port}/store.zarr")
        ds = xr.open_zarr(mapper, consolidated=True)
        full = ds[ZARR_VAR_NAME].load()  # whole array over HTTP
        snap = accumulator.snapshot()
        sub = full.sel(lat=slice(lat_min, lat_max), lon=slice(lon_min, lon_max))
        return _summary(sub.values), snap["total_bytes"], snap["n_requests"]

    if fmt == "cog":
        import rioxarray  # noqa: F401

        _set_gdal_vsicurl_env()
        accumulator.reset()
        url = f"/vsicurl/http://127.0.0.1:{port}/{cog_rel}"
        full = rioxarray.open_rasterio(url).load()  # whole raster over HTTP
        snap = accumulator.snapshot()
        clipped = full.rio.clip_box(
            minx=lon_min, miny=lat_min, maxx=lon_max, maxy=lat_max
        )
        return _summary(clipped.values), snap["total_bytes"], snap["n_requests"]

    raise ValueError(f"unknown fmt {fmt!r}")


def gate_remote(summaries: dict[str, dict], tol: float = 1e-3) -> dict:
    """Assert all contender summaries agree within ``tol``; return a record.

    ``count`` must match EXACTLY across contenders (same cell set); ``sum``,
    ``max`` and ``min`` must agree within ``tol``. A disagreement is returned as
    ``ok=False`` (a finding) rather than raised, so callers can record it.
    """
    names = list(summaries)
    if len(names) < 2:
        raise ValueError("gate_remote needs at least two summaries to compare")

    ref_name = names[0]
    ref = summaries[ref_name]
    ok = True
    max_abs_diff = 0.0
    detail: list[dict] = []
    for name in names[1:]:
        other = summaries[name]
        count_ok = other["count"] == ref["count"]
        diffs = {
            field_name: abs(float(other[field_name]) - float(ref[field_name]))
            for field_name in ("sum", "max", "min")
            if np.isfinite(other[field_name]) and np.isfinite(ref[field_name])
        }
        field_max = max(diffs.values()) if diffs else 0.0
        max_abs_diff = max(max_abs_diff, field_max)
        pair_ok = count_ok and field_max <= tol
        ok = ok and pair_ok
        detail.append(
            {
                "pair": (ref_name, name),
                "count_ok": count_ok,
                "ref_count": ref["count"],
                "other_count": other["count"],
                "max_abs_diff": field_max,
                "diffs": diffs,
                "ok": pair_ok,
            }
        )
    return {"ok": ok, "max_abs_diff": max_abs_diff, "tol": tol, "detail": detail}


if __name__ == "__main__":  # pragma: no cover - manual smoke entrypoint
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        info = generate_zarr(td, shape=(512, 512), chunks=(128, 128))
        print("zarr:", info["store"], info["shape"])
        cog = generate_cog(os.path.join(td, "cog.tif"), shape=(512, 512))
        print("cog:", cog["path"], cog["crs"])
        print("window 0.1:", window_bbox(info, 0.1))
