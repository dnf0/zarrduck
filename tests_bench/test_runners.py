"""Tests for the contender runners (Task 2).

Each runner maps ``(raster, polys, convention, metric)`` to ``{poly_id: float}``.
Conventions: ``centroid``, ``all_touched``, ``area_weighted``.
Metrics: ``max``, ``mean``, ``count``.

These run on a small "coarse" case (~50 sub-cell polygons over a small COG).
The eider runner needs the loadable extension built at
``target/debug/eider.duckdb_extension``; its tests are skip-guarded if it is
absent, but in CI/dev it should be built and genuinely run.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import pytest

# Make the repo root importable so `scripts.bench_zonal_headtohead` resolves.
REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

EXTENSION_PATH = REPO_ROOT / "target" / "debug" / "eider.duckdb_extension"


def _is_finite(value: float) -> bool:
    return isinstance(value, float) and math.isfinite(value)


@pytest.fixture(scope="module")
def coarse_case(tmp_path_factory):
    """Generate a small coarse-regime case once for all runner tests."""
    from scripts.bench_zonal_headtohead import generate_data

    out_dir = tmp_path_factory.mktemp("coarse_case")
    # Small raster keeps the spatial join cheap while still exercising the path.
    data = generate_data(out_dir, "coarse", 50, seed=7, shape=(80, 80))
    # Export the allow-path so eider reads are permitted under the data dir.
    import os

    os.environ["GEOZARR_ALLOW_PATH"] = str(out_dir)
    return data


# --------------------------------------------------------------------------
# rasterstats
# --------------------------------------------------------------------------
def test_rasterstats_all_touched_max(coarse_case):
    import geopandas as gpd

    from scripts.bench_zonal_headtohead import run_rasterstats

    poly_ids = list(gpd.read_parquet(coarse_case["parquet"])["poly_id"])
    res = run_rasterstats(
        coarse_case["raster"], coarse_case["geojson"], "all_touched", "max", poly_ids
    )
    assert isinstance(res, dict)
    assert set(res).issubset(set(poly_ids))
    finite = [v for v in res.values() if _is_finite(v)]
    assert finite, "all_touched max should yield finite values for every polygon"
    assert len(finite) == len(poly_ids)


def test_rasterstats_centroid_max_subset(coarse_case):
    import geopandas as gpd

    from scripts.bench_zonal_headtohead import run_rasterstats

    poly_ids = list(gpd.read_parquet(coarse_case["parquet"])["poly_id"])
    res = run_rasterstats(
        coarse_case["raster"], coarse_case["geojson"], "centroid", "max", poly_ids
    )
    assert isinstance(res, dict)
    # Centroid (cell-center rasterize) of sub-cell polygons is sparse: a subset
    # may be NaN, but every non-NaN value must be finite.
    assert all(_is_finite(v) for v in res.values() if not math.isnan(v))


def test_rasterstats_area_weighted_raises(coarse_case):
    from scripts.bench_zonal_headtohead import run_rasterstats

    with pytest.raises(NotImplementedError):
        run_rasterstats(
            coarse_case["raster"], coarse_case["geojson"], "area_weighted", "mean", [0]
        )


# --------------------------------------------------------------------------
# exactextract
# --------------------------------------------------------------------------
def test_exactextract_all_touched_max(coarse_case):
    import geopandas as gpd

    from scripts.bench_zonal_headtohead import run_exactextract

    gdf = gpd.read_parquet(coarse_case["parquet"])
    res = run_exactextract(coarse_case["raster"], gdf, "all_touched", "max")
    assert isinstance(res, dict)
    finite = [v for v in res.values() if _is_finite(v)]
    assert len(finite) == len(gdf), "every polygon should get a finite max"


def test_exactextract_area_weighted_mean(coarse_case):
    import geopandas as gpd

    from scripts.bench_zonal_headtohead import run_exactextract

    gdf = gpd.read_parquet(coarse_case["parquet"])
    res = run_exactextract(coarse_case["raster"], gdf, "area_weighted", "mean")
    assert isinstance(res, dict)
    finite = [v for v in res.values() if _is_finite(v)]
    assert len(finite) == len(gdf), "every polygon should get a finite mean"


# --------------------------------------------------------------------------
# eider
# --------------------------------------------------------------------------
@pytest.mark.skipif(
    not EXTENSION_PATH.exists(),
    reason=f"eider extension not built at {EXTENSION_PATH}",
)
def test_eider_all_touched_max(coarse_case):
    from scripts.bench_zonal_headtohead import eider_conn, run_eider

    conn = eider_conn()
    res = run_eider(
        conn, coarse_case["raster"], coarse_case["parquet"], "all_touched", "max"
    )
    assert isinstance(res, dict)
    finite = [v for v in res.values() if _is_finite(v)]
    assert finite, "all_touched max should yield finite values"
    # all_touched envelopes always intersect at least one cell per polygon.
    import geopandas as gpd

    poly_ids = list(gpd.read_parquet(coarse_case["parquet"])["poly_id"])
    assert len(finite) == len(poly_ids)


@pytest.mark.skipif(
    not EXTENSION_PATH.exists(),
    reason=f"eider extension not built at {EXTENSION_PATH}",
)
def test_eider_centroid_max_subset(coarse_case):
    from scripts.bench_zonal_headtohead import eider_conn, run_eider

    conn = eider_conn()
    res = run_eider(
        conn, coarse_case["raster"], coarse_case["parquet"], "centroid", "max"
    )
    assert isinstance(res, dict)
    assert all(_is_finite(v) for v in res.values())


@pytest.mark.skipif(
    not EXTENSION_PATH.exists(),
    reason=f"eider extension not built at {EXTENSION_PATH}",
)
def test_eider_area_weighted_mean(coarse_case):
    from scripts.bench_zonal_headtohead import eider_conn, run_eider

    conn = eider_conn()
    res = run_eider(
        conn, coarse_case["raster"], coarse_case["parquet"], "area_weighted", "mean"
    )
    assert isinstance(res, dict)
    assert all(_is_finite(v) for v in res.values())


@pytest.mark.skipif(
    not EXTENSION_PATH.exists(),
    reason=f"eider extension not built at {EXTENSION_PATH}",
)
def test_eider_indexjoin_max(coarse_case):
    from scripts.bench_zonal_headtohead import eider_conn, run_eider_indexjoin

    conn = eider_conn()
    res = run_eider_indexjoin(
        conn, coarse_case["raster"], coarse_case["parquet"], "centroid", "max"
    )
    assert isinstance(res, dict)
    assert all(_is_finite(v) for v in res.values())
