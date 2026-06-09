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
import sys
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
        # Default HTTP/1.0 closes each connection after responding, so pooled
        # client sockets (GDAL /vsicurl keep-alive) don't leave daemon reader
        # threads blocked on recv at interpreter shutdown.

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

            # Clients (GDAL/vsicurl, opendal) may close a ranged connection early;
            # swallow the resulting broken-pipe so the daemon thread doesn't emit
            # a teardown traceback. Bytes are recorded only once the write lands.
            try:
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
            except (BrokenPipeError, ConnectionResetError):
                pass

        def finish(self) -> None:
            # Swallow broken-pipe/reset on connection teardown so a client that
            # closed a ranged read early doesn't surface a daemon-thread
            # traceback at interpreter shutdown.
            try:
                super().finish()
            except (BrokenPipeError, ConnectionResetError, ValueError):
                pass

        def handle_one_request(self) -> None:
            try:
                super().handle_one_request()
            except (BrokenPipeError, ConnectionResetError):
                self.close_connection = True

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
    class _QuietServer(ThreadingHTTPServer):
        # Reap in-flight request threads on shutdown so they don't write to a
        # closed stdout during interpreter teardown (noisy excepthook errors).
        daemon_threads = True

        def handle_error(self, request, client_address) -> None:
            # Client-side early close on a ranged read is expected; stay quiet.
            pass

    server = _QuietServer(("127.0.0.1", 0), handler)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, port, accumulator


# --- Generators ------------------------------------------------------------


# Supported Zarr layouts the generator can emit. ``v2`` writes classic
# ``.zarray`` metadata; ``v3``/``v3_sharded`` write the production ``zarr.json``
# (native ``dimension_names``), the sharded variant adding a sharding codec so a
# single shard file holds many inner chunks.
ZARR_FORMATS = ("v2", "v3", "v3_sharded")

# Sharded-v3 shard-index placement. The Zarr v3 sharding codec may store its
# chunk index at the START or END of each shard file. eider's reader fetches the
# index correctly when it is at the START, but mis-ranges the trailing-index
# (``"end"``) form over HTTP — a partial read returns the whole shard where the
# 64-byte index was expected, tripping the crc32c check ("the checksum is
# invalid") / a chunk-size mismatch. We therefore write ``index_location="start"``
# so eider genuinely prunes sharded stores over HTTP. (This is the documented
# eider limitation surfaced by this benchmark; the layout is still a fully
# spec-compliant ``sharding_indexed`` store with real shard files.)
ZARR_V3_SHARD_INDEX_LOCATION = "start"


def _write_zarr_v3_sharded(store_path, ds, *, chunks, shards) -> None:
    """Write ``ds`` as a consolidated Zarr v3 store using the sharding codec.

    xarray's ``to_zarr`` encoding cannot set the shard ``index_location``, so the
    sharded variable is written directly via the zarr API: the array chunk grid
    is the *shard* shape and the sharding codec's inner ``chunk_shape`` is
    ``chunks``, so each shard file packs ``(shards/chunks)`` inner chunks. lat/lon
    coordinate arrays are written too, then metadata is consolidated so the store
    opens over plain HTTP.
    """
    import zarr
    from zarr.codecs import BytesCodec, ShardingCodec, ZstdCodec

    var = ds[ZARR_VAR_NAME]
    lat = ds["lat"].values
    lon = ds["lon"].values

    group = zarr.open_group(str(store_path), mode="w", zarr_format=3, attributes={})
    sharding = ShardingCodec(
        chunk_shape=tuple(chunks),
        codecs=[BytesCodec(), ZstdCodec(level=0)],
        index_location=ZARR_V3_SHARD_INDEX_LOCATION,
    )
    arr = group.create_array(
        ZARR_VAR_NAME,
        shape=var.shape,
        dtype=var.dtype,
        chunks=tuple(shards),
        serializer=sharding,
        compressors=[],
        dimension_names=("lat", "lon"),
    )
    arr[:] = var.values

    lat_arr = group.create_array(
        "lat", shape=lat.shape, dtype=lat.dtype, chunks=lat.shape, dimension_names=("lat",)
    )
    lat_arr[:] = lat
    lon_arr = group.create_array(
        "lon", shape=lon.shape, dtype=lon.dtype, chunks=lon.shape, dimension_names=("lon",)
    )
    lon_arr[:] = lon

    zarr.consolidate_metadata(group.store)


def generate_zarr(
    out_dir,
    shape: tuple[int, int] = (4000, 4000),
    chunks: tuple[int, int] = (256, 256),
    seed: int = 42,
    fmt: str = "v3",
    shards: Optional[tuple[int, int]] = None,
) -> dict:
    """Generate a 2D float32 Zarr store with lat/lon coordinate arrays.

    Values are a smooth gradient plus reproducible noise. Written via
    ``xarray.to_zarr`` (zarr 3.x writes both v2 and v3). ``fmt`` selects the
    on-disk layout:

      * ``"v2"`` -> ``zarr_format=2`` classic metadata (``.zarray``/``.zattrs``),
        consolidated ``.zmetadata``. Uses ``_ARRAY_DIMENSIONS`` for dim names.
      * ``"v3"`` -> ``zarr_format=3`` native metadata (``zarr.json`` with
        ``dimension_names``); one chunk file per chunk.
      * ``"v3_sharded"`` -> ``zarr_format=3`` with a sharding codec so each shard
        file packs multiple inner chunks. ``shards`` (default ``4*chunks`` per
        axis, clamped to ``shape``) must be a whole multiple of ``chunks``.

    lat/lon coordinate arrays are always written so eider prunes and xarray
    ``.sel`` resolves windows. Returns the store path + coordinate info.
    """
    import xarray as xr

    if fmt not in ZARR_FORMATS:
        raise ValueError(f"fmt must be one of {ZARR_FORMATS}, got {fmt!r}")

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
    if fmt == "v2":
        # v2: consolidated metadata (.zmetadata) + per-chunk .zarray files.
        ds.to_zarr(
            str(store_path),
            mode="w",
            consolidated=True,
            zarr_format=2,
            encoding={ZARR_VAR_NAME: {"chunks": chunks}},
        )
        var_encoding: dict = {"chunks": chunks}
    elif fmt == "v3":
        var_encoding = {"chunks": chunks}
        # Consolidated v3 metadata: writes a root ``zarr.json`` carrying a
        # ``consolidated_metadata`` block so the store can be opened over plain
        # HTTP (no directory listing) — the per-array ``zarr.json`` still uses
        # native ``dimension_names`` (no ``_ARRAY_DIMENSIONS``).
        ds.to_zarr(
            str(store_path),
            mode="w",
            zarr_format=3,
            consolidated=True,
            encoding={ZARR_VAR_NAME: var_encoding},
        )
    else:  # v3_sharded
        if shards is None:
            # Default each shard to 2x the chunk per axis (clamped to shape) so a
            # non-trivial store packs several shard files -> prunable.
            shards = (min(chunks[0] * 2, height), min(chunks[1] * 2, width))
        for s, c in zip(shards, chunks):
            if s % c != 0:
                raise ValueError(
                    f"shards {shards} must be whole multiples of chunks {chunks}"
                )
        var_encoding = {"chunks": chunks, "shards": shards}
        _write_zarr_v3_sharded(store_path, ds, chunks=chunks, shards=shards)

    return {
        "store": store_path,
        "var": ZARR_VAR_NAME,
        "fmt": fmt,
        "shape": shape,
        "chunks": chunks,
        "shards": var_encoding.get("shards"),
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


def _cog_centre_values(clipped, bbox) -> np.ndarray:
    """Mask a clipped rioxarray window to cells whose CENTRES lie in the bbox.

    ``rio.clip_box`` keeps any pixel whose *extent* overlaps the bbox, so it
    includes a fringe of edge pixels whose centres fall just outside the window.
    eider's bbox pushdown is post-filtered to cell *centres* in the closed bbox.
    To make the two contenders select the identical cell set we drop the fringe
    here by filtering the clip's ``x``/``y`` centre coordinates to the closed
    bbox — exactly the predicate eider's COG runner applies.
    """
    lon_min, lat_min, lon_max, lat_max = bbox
    x = np.asarray(clipped["x"].values, dtype="float64")
    y = np.asarray(clipped["y"].values, dtype="float64")
    xmask = (x >= lon_min) & (x <= lon_max)
    ymask = (y >= lat_min) & (y <= lat_max)
    vals = np.asarray(clipped.values)
    # rioxarray bands -> squeeze to 2D (y, x) if single-band.
    if vals.ndim == 3 and vals.shape[0] == 1:
        vals = vals[0]
    return vals[np.ix_(ymask, xmask)]


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
    if fmt.startswith("zarr"):
        import fsspec
        import xarray as xr

        accumulator.reset()
        mapper = fsspec.get_mapper(f"http://127.0.0.1:{port}/store.zarr")
        # All formats (v2, v3, v3_sharded) are written consolidated so the store
        # opens over plain HTTP without directory listing.
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
        values = _cog_centre_values(clipped, bbox)
        return _summary(values), snap["total_bytes"], snap["n_requests"]

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
    if fmt.startswith("zarr"):
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
        values = _cog_centre_values(clipped, bbox)
        return _summary(values), snap["total_bytes"], snap["n_requests"]

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


# --- Matrix + emit + CLI (Task 3) -----------------------------------------
#
# The matrix runs format x window x contender. bytes/requests are deterministic
# (one measure); wall time is the median of ``reps`` reads (a labeled localhost
# footnote, NOT the headline). The correctness gate runs once per (format,
# window) over the three contender summaries before any timing is trusted.

import argparse  # noqa: E402
import json  # noqa: E402
import statistics  # noqa: E402
import tempfile  # noqa: E402
import time  # noqa: E402

# Formats the matrix can cover. The three Zarr layouts share one served store
# per format; ``cog`` is a tiled GeoTIFF addressed via its STAC item asset.
MATRIX_FORMATS = ("zarr_v2", "zarr_v3", "zarr_v3_sharded", "cog")
DEFAULT_WINDOWS = (0.001, 0.01, 0.1)
DEFAULT_SHAPE = (4000, 4000)
DEFAULT_CHUNKS = (256, 256)
QUICK_SHAPE = (1024, 1024)
QUICK_CHUNKS = (128, 128)
QUICK_WINDOWS = (0.01,)
CONTENDERS = ("eider", "chunk_aware", "naive")

# Default extension path (repo-relative); overridable via --extension.
DEFAULT_EXTENSION_PATH = (
    Path(__file__).resolve().parents[1] / "target" / "debug" / "eider.duckdb_extension"
)


def _zarr_fmt_to_generator(fmt: str) -> str:
    """Map a matrix format name (``zarr_v3``) to a generator ``fmt`` (``v3``)."""
    return fmt.removeprefix("zarr_")


def _env_block() -> dict:
    """Capture library versions + the localhost caveat for the emitted report."""
    import duckdb
    import zarr

    versions = {"python": sys.version.split()[0], "duckdb": duckdb.__version__, "zarr": zarr.__version__}
    for name in ("xarray", "fsspec", "numpy", "rioxarray", "rasterio"):
        try:
            versions[name] = __import__(name).__version__
        except Exception:  # pragma: no cover - optional dep missing
            versions[name] = "n/a"
    return {
        "versions": versions,
        "caveat": (
            "Bytes/requests are measured over a LOCALHOST Range-capable HTTP "
            "server against a SYNTHETIC store; wall times are localhost-only and "
            "are a footnote, not the headline. Bytes fetched + request count are "
            "the deterministic, transport-independent result."
        ),
    }


def _measure_time(fn, reps: int) -> float:
    """Return the median wall time (seconds) over ``reps`` calls of ``fn``."""
    times = []
    for _ in range(max(1, reps)):
        t0 = time.perf_counter()
        fn()
        times.append(time.perf_counter() - t0)
    return statistics.median(times)


def _run_one_cell(fmt, contender, ctx, extension_path, reps):
    """Run a single (format, contender) cell: summary, bytes, requests, time.

    Returns ``(summary, bytes, n_requests, seconds, error)``. ``error`` is a
    string when the contender could not read the store (recorded, not hidden).
    """
    port = ctx["port"]
    acc = ctx["acc"]
    bbox = ctx["bbox"]

    def call():
        if contender == "eider":
            return run_eider_remote(
                port, ctx["store_path"], bbox, acc, extension_path, fmt=fmt
            )
        cog_rel = ctx.get("cog_rel", "grid.tif")
        if contender == "chunk_aware":
            return run_chunkaware_remote(port, bbox, acc, fmt=fmt, cog_rel=cog_rel)
        return run_naive_remote(port, bbox, acc, fmt=fmt, cog_rel=cog_rel)

    try:
        summary, n_bytes, n_req = call()
    except Exception as exc:  # contender genuinely failed on this store
        return None, 0, 0, 0.0, f"{type(exc).__name__}: {exc}"

    seconds = _measure_time(lambda: call(), reps) if reps > 1 else _measure_time(call, 1)
    return summary, n_bytes, n_req, seconds, None


def run_matrix(
    out_dir,
    *,
    formats=MATRIX_FORMATS,
    windows=DEFAULT_WINDOWS,
    shape=DEFAULT_SHAPE,
    chunks=DEFAULT_CHUNKS,
    reps: int = 3,
    extension_path=None,
) -> dict:
    """Run the format x window x contender matrix; return a results record.

    For each format a store is generated once and served from its own root. For
    each window the three contenders read the same window; the correctness gate
    runs over their summaries, then bytes/requests/time are recorded per cell.
    Eider failures are recorded as ``error`` (not hidden) and fail that gate.
    """
    out_dir = Path(out_dir)
    extension_path = Path(extension_path or DEFAULT_EXTENSION_PATH)
    rows: list[dict] = []
    gates: list[dict] = []

    for fmt in formats:
        root = out_dir / f"store_{fmt}"
        root.mkdir(parents=True, exist_ok=True)

        if fmt == "cog":
            info = generate_cog(root / "grid.tif", shape=shape, blocksize=chunks[0])
        else:
            info = generate_zarr(
                root, shape=shape, chunks=chunks, fmt=_zarr_fmt_to_generator(fmt)
            )

        server, port, acc = start_server(root)
        os.environ["GEOZARR_ALLOW_PATH"] = str(root)
        try:
            if fmt == "cog":
                cog_item_rel = write_cog_stac_item(root, port, cog_rel="grid.tif")
                store_path = f"{cog_item_rel}/{COG_ASSET_NAME}"
                cog_rel = "grid.tif"
            else:
                store_path = f"store.zarr/{ZARR_VAR_NAME}"
                cog_rel = "grid.tif"

            for window in windows:
                bbox = window_bbox(info, window)
                ctx = {
                    "port": port,
                    "acc": acc,
                    "bbox": bbox,
                    "store_path": store_path,
                    "cog_rel": cog_rel,
                }
                cells: dict[str, dict] = {}
                summaries: dict[str, dict] = {}
                for contender in CONTENDERS:
                    summary, n_bytes, n_req, seconds, error = _run_one_cell(
                        fmt, contender, ctx, extension_path, reps
                    )
                    cells[contender] = {
                        "bytes": n_bytes,
                        "requests": n_req,
                        "seconds": seconds,
                        "error": error,
                    }
                    if summary is not None:
                        summaries[contender] = summary

                if len(summaries) >= 2:
                    gate = gate_remote(summaries)
                else:
                    gate = {"ok": False, "reason": "fewer than two contenders read"}
                gates.append({"format": fmt, "window": window, **gate})

                naive_bytes = cells["naive"]["bytes"] or None
                for contender in CONTENDERS:
                    c = cells[contender]
                    ratio = (
                        c["bytes"] / naive_bytes
                        if naive_bytes and c["error"] is None
                        else None
                    )
                    rows.append(
                        {
                            "format": fmt,
                            "window": window,
                            "contender": contender,
                            "bytes": c["bytes"],
                            "requests": c["requests"],
                            "seconds": c["seconds"],
                            "bytes_over_naive": ratio,
                            "error": c["error"],
                        }
                    )
        finally:
            server.shutdown()
            server.server_close()

    return {
        "env": _env_block(),
        "config": {
            "formats": list(formats),
            "windows": list(windows),
            "shape": list(shape),
            "chunks": list(chunks),
            "reps": reps,
        },
        "rows": rows,
        "gates": gates,
    }


def format_table(results: dict) -> str:
    """Render the matrix rows as a fixed-width stdout table."""
    headers = ["format", "window", "contender", "bytes", "requests", "time(s)", "÷naive"]
    lines = [["" if v is None else v for v in headers]]
    for r in results["rows"]:
        ratio = r["bytes_over_naive"]
        ratio_s = "error" if r["error"] else (f"{ratio:.3f}" if ratio is not None else "-")
        lines.append(
            [
                r["format"],
                f"{r['window']:g}",
                r["contender"],
                "err" if r["error"] else f"{r['bytes']:,}",
                "err" if r["error"] else f"{r['requests']}",
                f"{r['seconds']:.4f}",
                ratio_s,
            ]
        )
    widths = [max(len(str(row[i])) for row in lines) for i in range(len(headers))]
    out = []
    for ri, row in enumerate(lines):
        out.append("  ".join(str(c).ljust(widths[i]) for i, c in enumerate(row)))
        if ri == 0:
            out.append("  ".join("-" * widths[i] for i in range(len(headers))))
    # Gate summary + any contender errors.
    out.append("")
    for g in results["gates"]:
        status = "PASS" if g.get("ok") else "FAIL"
        out.append(
            f"gate {g['format']} window={g['window']:g}: {status}"
            + (f" ({g['reason']})" if g.get("reason") else "")
        )
    errors = [r for r in results["rows"] if r["error"]]
    if errors:
        out.append("")
        out.append("contender errors (recorded, not hidden):")
        for r in errors:
            out.append(f"  {r['format']} {r['contender']} w={r['window']:g}: {r['error']}")
    out.append("")
    out.append(f"env: {json.dumps(results['env']['versions'])}")
    out.append(f"caveat: {results['env']['caveat']}")
    return "\n".join(out)


def _build_arg_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="Remote partial-read benchmark: eider vs chunk-aware vs naive "
        "over a Range-capable localhost HTTP server, across Zarr v2/v3/v3_sharded "
        "and COG.",
    )
    p.add_argument("--out-dir", default=None, help="store directory (default: temp dir)")
    p.add_argument(
        "--windows",
        type=float,
        nargs="+",
        default=list(DEFAULT_WINDOWS),
        help="window area fractions of the grid (e.g. 0.001 0.01 0.1)",
    )
    p.add_argument(
        "--shape",
        type=int,
        nargs=2,
        default=list(DEFAULT_SHAPE),
        metavar=("HEIGHT", "WIDTH"),
        help="grid shape (rows cols)",
    )
    p.add_argument(
        "--formats",
        nargs="+",
        choices=MATRIX_FORMATS,
        default=list(MATRIX_FORMATS),
        help="formats to benchmark",
    )
    p.add_argument("--reps", type=int, default=3, help="timing reps (median)")
    p.add_argument("--json", dest="json_out", default=None, help="write results JSON here")
    p.add_argument(
        "--quick",
        action="store_true",
        help="tiny store, all formats, one window — fast end-to-end self-test",
    )
    p.add_argument(
        "--extension",
        default=str(DEFAULT_EXTENSION_PATH),
        help="path to the eider duckdb extension",
    )
    return p


def _silence_shutdown_thread_errors() -> None:
    """Swallow daemon-thread teardown tracebacks at interpreter shutdown.

    The Range server runs request handlers in daemon threads; when the
    interpreter exits, threads blocked on a client socket are torn down and the
    default ``threading.excepthook`` tries to print to an already-closing stdout,
    producing cosmetic ``Error in sys.excepthook`` noise. The functional output
    has already been flushed by then, so we drop these.
    """

    def hook(args) -> None:  # noqa: ANN001
        if args.exc_type in (BrokenPipeError, ConnectionResetError, ValueError):
            return
        sys.__excepthook__(args.exc_type, args.exc_value, args.exc_traceback)

    threading.excepthook = hook


def main(argv=None) -> int:
    _silence_shutdown_thread_errors()
    args = _build_arg_parser().parse_args(argv)

    shape = tuple(args.shape)
    chunks = DEFAULT_CHUNKS
    windows = tuple(args.windows)
    formats = tuple(args.formats)
    if args.quick:
        shape = QUICK_SHAPE
        chunks = QUICK_CHUNKS
        windows = QUICK_WINDOWS

    def _go(out_dir) -> int:
        results = run_matrix(
            out_dir,
            formats=formats,
            windows=windows,
            shape=shape,
            chunks=chunks,
            reps=args.reps,
            extension_path=args.extension,
        )
        print(format_table(results))
        if args.json_out:
            Path(args.json_out).write_text(json.dumps(results, indent=2), encoding="utf-8")
            print(f"\nwrote JSON -> {args.json_out}")
        sys.stdout.flush()
        all_ok = all(g.get("ok") for g in results["gates"])
        return 0 if all_ok else 1

    if args.out_dir:
        return _go(args.out_dir)
    with tempfile.TemporaryDirectory() as td:
        return _go(td)


if __name__ == "__main__":
    raise SystemExit(main())
