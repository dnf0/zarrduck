"""Tests for the remote partial-read contender runners + correctness gate (Task 2).

A single small store (512x512 Zarr, chunks 128; 512x512 COG, tile 128) is served
over the Range-capable byte-logging HTTP server. One ~10%-area centered window is
read by each contender (eider / chunk-aware / naive). The gate asserts the three
window summaries agree; bytes accounting confirms eider and chunk-aware fetch
FEWER bytes than the naive whole-store read.

eider genuinely runs over HTTP (no mocking). The eider COG path is exercised via
the single-Feature STAC-item asset URL because the bare ``.tif`` HTTP form is not
yet readable by the extension (recorded in the run report).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.bench_remote_partialread import (  # noqa: E402
    COG_ASSET_NAME,
    ZARR_VAR_NAME,
    gate_remote,
    generate_cog,
    generate_zarr,
    run_chunkaware_remote,
    run_eider_remote,
    run_naive_remote,
    start_server,
    window_bbox,
    write_cog_stac_item,
)

EXTENSION_PATH = REPO_ROOT / "target" / "debug" / "eider.duckdb_extension"
WINDOW_FRACTION = 0.1


@pytest.fixture(scope="module")
def served_store(tmp_path_factory):
    """Generate the small Zarr + COG, serve them, and yield the read context."""
    root = tmp_path_factory.mktemp("remote_store")
    info = generate_zarr(root, shape=(512, 512), chunks=(128, 128), seed=1)
    generate_cog(root / "grid.tif", shape=(512, 512), blocksize=128, seed=1)

    server, port, acc = start_server(root)
    # eider's local-path sandbox gate also covers the served STAC item it reads.
    os.environ["GEOZARR_ALLOW_PATH"] = str(root)
    cog_item_rel = write_cog_stac_item(root, port, cog_rel="grid.tif")

    bbox = window_bbox(info, WINDOW_FRACTION)
    try:
        yield {
            "port": port,
            "acc": acc,
            "bbox": bbox,
            "zarr_store_path": f"store.zarr/{ZARR_VAR_NAME}",
            "cog_store_path": f"{cog_item_rel}/{COG_ASSET_NAME}",
        }
    finally:
        server.shutdown()


# --------------------------------------------------------------------------
# Zarr: all three contenders agree, and the pruning contenders fetch less.
# --------------------------------------------------------------------------
@pytest.mark.skipif(
    not EXTENSION_PATH.exists(), reason=f"eider extension not built at {EXTENSION_PATH}"
)
def test_zarr_three_way_agree_and_prune(served_store):
    port = served_store["port"]
    acc = served_store["acc"]
    bbox = served_store["bbox"]

    eider_sum, eider_bytes, eider_req = run_eider_remote(
        port, served_store["zarr_store_path"], bbox, acc, EXTENSION_PATH, fmt="zarr"
    )
    ca_sum, ca_bytes, _ = run_chunkaware_remote(port, bbox, acc, fmt="zarr")
    naive_sum, naive_bytes, _ = run_naive_remote(port, bbox, acc, fmt="zarr")

    # eider genuinely fetched bytes over HTTP for the window.
    assert eider_req > 0
    assert eider_bytes > 0

    rep = gate_remote(
        {"eider": eider_sum, "chunk_aware": ca_sum, "naive": naive_sum}, tol=1e-3
    )
    assert rep["ok"], rep

    # Same cell set across all three.
    assert eider_sum["count"] == ca_sum["count"] == naive_sum["count"]
    assert eider_sum["count"] > 0

    # The pruning contenders fetch strictly fewer bytes than the naive read.
    assert eider_bytes < naive_bytes
    assert ca_bytes < naive_bytes


# --------------------------------------------------------------------------
# COG: all three contenders agree, and the pruning contenders fetch less.
# --------------------------------------------------------------------------
@pytest.mark.skipif(
    not EXTENSION_PATH.exists(), reason=f"eider extension not built at {EXTENSION_PATH}"
)
def test_cog_three_way_agree_and_prune(served_store):
    port = served_store["port"]
    acc = served_store["acc"]
    bbox = served_store["bbox"]

    eider_sum, eider_bytes, eider_req = run_eider_remote(
        port, served_store["cog_store_path"], bbox, acc, EXTENSION_PATH, fmt="cog"
    )
    ca_sum, ca_bytes, _ = run_chunkaware_remote(port, bbox, acc, fmt="cog")
    naive_sum, naive_bytes, _ = run_naive_remote(port, bbox, acc, fmt="cog")

    # eider genuinely fetched bytes over HTTP (ranged GETs) for the window.
    assert eider_req > 0
    assert eider_bytes > 0

    rep = gate_remote(
        {"eider": eider_sum, "chunk_aware": ca_sum, "naive": naive_sum}, tol=1e-3
    )
    assert rep["ok"], rep

    assert eider_sum["count"] == ca_sum["count"] == naive_sum["count"]
    assert eider_sum["count"] > 0

    assert eider_bytes < naive_bytes
    assert ca_bytes < naive_bytes


# --------------------------------------------------------------------------
# Gate unit behaviour: an honest disagreement is a returned finding, not a crash.
# --------------------------------------------------------------------------
def test_gate_detects_count_mismatch():
    a = {"count": 100, "sum": 50.0, "max": 1.0, "min": 0.0}
    b = {"count": 99, "sum": 50.0, "max": 1.0, "min": 0.0}
    rep = gate_remote({"a": a, "b": b}, tol=1e-3)
    assert rep["ok"] is False
    assert rep["detail"][0]["count_ok"] is False


def test_gate_agrees_within_tol():
    a = {"count": 100, "sum": 50.0000, "max": 1.0, "min": 0.0}
    b = {"count": 100, "sum": 50.0005, "max": 1.0, "min": 0.0}
    rep = gate_remote({"a": a, "b": b}, tol=1e-3)
    assert rep["ok"] is True
    assert rep["max_abs_diff"] <= 1e-3
