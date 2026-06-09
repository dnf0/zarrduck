"""Tests for the Range-capable byte-logging server + remote store generators."""

import os
import sys
from pathlib import Path

import requests

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.bench_remote_partialread import (  # noqa: E402
    generate_cog,
    generate_zarr,
    start_server,
    window_bbox,
)


def test_server_full_get(tmp_path):
    payload = b"0123456789" * 50  # 500 bytes
    (tmp_path / "data.bin").write_bytes(payload)
    server, port, acc = start_server(tmp_path)
    try:
        acc.reset()
        resp = requests.get(f"http://127.0.0.1:{port}/data.bin", timeout=5)
        assert resp.status_code == 200
        assert resp.content == payload
        snap = acc.snapshot()
        assert snap["n_requests"] == 1
        assert snap["total_bytes"] == len(payload)
        assert snap["per_path"]["/data.bin"]["requests"] == 1
    finally:
        server.shutdown()


def test_server_range_get(tmp_path):
    payload = b"0123456789" * 50  # 500 bytes
    (tmp_path / "data.bin").write_bytes(payload)
    server, port, acc = start_server(tmp_path)
    try:
        acc.reset()
        resp = requests.get(
            f"http://127.0.0.1:{port}/data.bin",
            headers={"Range": "bytes=0-99"},
            timeout=5,
        )
        assert resp.status_code == 206
        assert len(resp.content) == 100
        assert resp.content == payload[:100]
        assert resp.headers["Content-Range"] == "bytes 0-99/500"
        snap = acc.snapshot()
        assert snap["n_requests"] == 1
        assert snap["total_bytes"] == 100
        # The recorded request is flagged as a range request.
        rec = snap["per_path"]["/data.bin"]
        assert rec["bytes"] == 100
    finally:
        server.shutdown()


def test_server_range_is_flagged(tmp_path):
    (tmp_path / "data.bin").write_bytes(b"x" * 200)
    server, port, acc = start_server(tmp_path)
    try:
        acc.reset()
        requests.get(
            f"http://127.0.0.1:{port}/data.bin",
            headers={"Range": "bytes=10-19"},
            timeout=5,
        )
        # Reach into the accumulator records to confirm is_range=True.
        records = acc._records  # noqa: SLF001 (test introspection)
        assert len(records) == 1
        assert records[0].is_range is True
    finally:
        server.shutdown()


def _read_array_zarr_json(store, var):
    """Load the per-array v3 ``zarr.json`` for ``var`` from a store on disk."""
    import json

    path = os.path.join(store, var, "zarr.json")
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def test_generate_zarr_v2_reopens(tmp_path):
    import numpy as np
    import xarray as xr

    info = generate_zarr(tmp_path, shape=(512, 512), chunks=(128, 128), seed=1, fmt="v2")
    assert info["fmt"] == "v2"
    ds = xr.open_zarr(str(info["store"]))
    var = ds[info["var"]]
    assert var.shape == (512, 512)
    assert var.dtype == "float32"
    assert "lat" in ds.coords and "lon" in ds.coords
    # Coordinates are monotonic ascending.
    assert np.all(np.diff(ds["lat"].values) > 0)
    assert np.all(np.diff(ds["lon"].values) > 0)


def test_generate_zarr_v2_is_v2_not_v3(tmp_path):
    info = generate_zarr(tmp_path, shape=(256, 256), chunks=(128, 128), seed=1, fmt="v2")
    store = info["store"]
    found_zarray = []
    found_zarr_json = []
    for root, _dirs, files in os.walk(store):
        for f in files:
            if f == ".zarray":
                found_zarray.append(os.path.join(root, f))
            if f == "zarr.json":
                found_zarr_json.append(os.path.join(root, f))
    # v2: expect classic .zarray files, NOT v3 zarr.json.
    assert found_zarray, "expected zarr v2 .zarray metadata"
    assert not found_zarr_json, "found zarr v3 zarr.json; must be v2"


def test_generate_zarr_v3_reopens(tmp_path):
    import numpy as np
    import xarray as xr

    info = generate_zarr(tmp_path, shape=(512, 512), chunks=(128, 128), seed=1, fmt="v3")
    assert info["fmt"] == "v3"
    assert info["shards"] is None
    ds = xr.open_zarr(str(info["store"]), consolidated=True)
    var = ds[info["var"]]
    assert var.shape == (512, 512)
    assert var.dtype == "float32"
    assert np.all(np.diff(ds["lat"].values) > 0)
    assert np.all(np.diff(ds["lon"].values) > 0)


def test_generate_zarr_v3_native_metadata(tmp_path):
    info = generate_zarr(tmp_path, shape=(256, 256), chunks=(128, 128), seed=1, fmt="v3")
    store = info["store"]
    # v3: per-array zarr.json present, NO classic .zarray anywhere.
    found_zarray = []
    found_zarr_json = []
    for root, _dirs, files in os.walk(store):
        for f in files:
            if f == ".zarray":
                found_zarray.append(os.path.join(root, f))
            if f == "zarr.json":
                found_zarr_json.append(os.path.join(root, f))
    assert not found_zarray, "found classic .zarray; v3 must use zarr.json"
    assert found_zarr_json, "expected v3 zarr.json metadata"

    # The variable array carries native dimension_names, NOT v2 _ARRAY_DIMENSIONS.
    array_meta = _read_array_zarr_json(store, info["var"])
    assert array_meta["dimension_names"] == ["lat", "lon"]
    assert "_ARRAY_DIMENSIONS" not in array_meta.get("attributes", {})


def test_generate_zarr_v3_sharded_layout(tmp_path):
    info = generate_zarr(
        tmp_path, shape=(512, 512), chunks=(128, 128), seed=1, fmt="v3_sharded"
    )
    assert info["fmt"] == "v3_sharded"
    # Default shards = 2x chunks per axis.
    assert info["shards"] == (256, 256)

    store = info["store"]
    array_meta = _read_array_zarr_json(store, info["var"])
    # Native v3 dimension names, no v2 attribute.
    assert array_meta["dimension_names"] == ["lat", "lon"]
    assert "_ARRAY_DIMENSIONS" not in array_meta.get("attributes", {})

    # The variable uses the sharding codec; the array chunk grid is the SHARD
    # shape and the codec's inner chunk_shape is the (smaller) chunk shape.
    codecs = array_meta["codecs"]
    assert codecs[0]["name"] == "sharding_indexed"
    cfg = codecs[0]["configuration"]
    assert cfg["chunk_shape"] == [128, 128]
    assert array_meta["chunk_grid"]["configuration"]["chunk_shape"] == [256, 256]

    # Multiple shard files exist under c/ (512/256 = 2 shards per axis -> 4).
    shard_dir = os.path.join(store, info["var"], "c")
    shard_files = [
        os.path.join(r, f) for r, _d, fs in os.walk(shard_dir) for f in fs
    ]
    assert len(shard_files) == 4, f"expected 4 shard files, got {len(shard_files)}"


def test_generate_zarr_v3_sharded_rejects_misaligned_shards(tmp_path):
    import pytest

    with pytest.raises(ValueError, match="whole multiples"):
        generate_zarr(
            tmp_path,
            shape=(512, 512),
            chunks=(128, 128),
            fmt="v3_sharded",
            shards=(200, 200),
        )


def test_generate_cog_reopens(tmp_path):
    import rasterio

    path = tmp_path / "cog.tif"
    generate_cog(path, shape=(512, 512), blocksize=256, seed=1)
    with rasterio.open(str(path)) as r:
        assert r.count == 1
        assert r.crs.to_epsg() == 4326
        assert r.width == 512
        assert r.height == 512
        assert r.dtypes[0] == "float32"
        # Tiled with the requested block size.
        assert r.profile.get("tiled") is True
        assert r.block_shapes[0] == (256, 256)
        # North-up: pixel height (transform.e) is negative.
        assert r.transform.e < 0


def test_window_bbox_centered():
    info = {"lon_min": -10.0, "lon_max": 10.0, "lat_min": -10.0, "lat_max": 10.0}
    lon_min, lat_min, lon_max, lat_max = window_bbox(info, 0.01)
    # Centered on (0, 0).
    assert abs((lon_min + lon_max) / 2.0) < 1e-9
    assert abs((lat_min + lat_max) / 2.0) < 1e-9
    # Area fraction ~ 0.01 => side fraction 0.1 => width 0.1 * 20 = 2.0 degrees.
    assert abs((lon_max - lon_min) - 2.0) < 1e-9
    assert abs((lat_max - lat_min) - 2.0) < 1e-9
    # Bbox stays within the grid.
    assert lon_min >= info["lon_min"] and lon_max <= info["lon_max"]
