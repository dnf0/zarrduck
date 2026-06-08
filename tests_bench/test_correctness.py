"""Cross-tool correctness gate (Task 3).

Runs the contender runners on a small "fine"-regime case and asserts that
eider/DuckDB agrees with rasterstats and exactextract per the MATCHING
convention/metric definitions. The whole benchmark's credibility rests on this
gate: tolerances are chosen from observed agreement, never relaxed to force a
pass. See the module-level NOTE on the half-pixel correction in
``scripts/bench_zonal_headtohead._field_cte`` for the reconciliation that made
the MAX/centroid pairings agree exactly.

Honest convention <-> tool mapping (definitions MUST match for a comparison):
  * centroid / max      : eider ST_Contains(cell-centre) == rasterstats
                          all_touched=False (GDAL cell-centre rasterize). MAX is
                          an exact cell selection -> agreement is exact.
  * all_touched / max   : eider cell-box ST_Intersects == rasterstats
                          all_touched=True == exactextract max (max over all
                          cells with coverage > 0). Exact cell selection -> exact.
  * all_touched / mean  : eider == rasterstats all_touched=True. Both average the
                          SAME set of fully-counted touched cells; the only gap is
                          float accumulation order -> a tiny absolute tolerance.
  * area_weighted / mean: eider sum(v*area)/sum(area) over cell boxes ==
                          exactextract coverage-weighted mean. Different
                          area-intersection math (exactextract's analytic
                          coverage vs DuckDB ST_Intersection area) -> a small
                          tolerance; observed agreement is ~1e-8.
  * count               : eider count(*) of touched cells == rasterstats
                          all_touched=True count. Both are INTEGER cell counts ->
                          exact. We deliberately do NOT compare exactextract
                          count here: exactextract's "count" is the SUM OF
                          COVERAGE FRACTIONS (fractional area, a different
                          definition), so comparing it to an integer cell count
                          would be meaningless.

Observed max_abs_diff on this data (seed=11, fine, n=300, 600x600 grid):
  centroid/max          0.0
  all_touched/max  (rs)  0.0
  all_touched/max  (xe)  0.0
  all_touched/mean       ~8.6e-06   (float accumulation)
  area_weighted/mean     ~1.2e-08   (area-intersection rounding)
  count                  0.0
Chosen tolerances sit comfortably above the observed diffs while staying tight
enough that a real convention bug (e.g. a half-pixel cell-box offset, which we
already found and fixed -> diffs of ~3-4) would fail the gate.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

EXTENSION_PATH = REPO_ROOT / "target" / "debug" / "eider.duckdb_extension"

# --- Tolerances per pairing (justified above) ------------------------------
TOL_MAX_EXACT = 1e-3  # exact cell selection; observed 0.0
TOL_ALL_TOUCHED_MEAN = 1e-2  # float accumulation order; observed ~8.6e-06
TOL_AREA_WEIGHTED_MEAN = 1e-1  # area-intersection math; observed ~1.2e-08
TOL_COUNT_EXACT = 1e-9  # integer cell counts; observed 0.0

# Modest fine-regime case: large enough to exercise multi-cell diamonds, small
# enough to run the spatial joins quickly in CI.
N_POLYS = 300
GRID_SHAPE = (600, 600)
SEED = 11

pytestmark = pytest.mark.skipif(
    not EXTENSION_PATH.exists(),
    reason=f"eider extension not built at {EXTENSION_PATH}",
)


@pytest.fixture(scope="module")
def fine_case(tmp_path_factory):
    """Generate one fine-regime case and an eider connection for the module."""
    from scripts.bench_zonal_headtohead import generate_data

    out_dir = tmp_path_factory.mktemp("fine_case")
    os.environ["GEOZARR_ALLOW_PATH"] = str(out_dir)
    data = generate_data(out_dir, "fine", N_POLYS, seed=SEED, shape=GRID_SHAPE)
    return data


@pytest.fixture(scope="module")
def conn():
    from scripts.bench_zonal_headtohead import eider_conn

    return eider_conn()


@pytest.fixture(scope="module")
def gdf(fine_case):
    import geopandas as gpd

    return gpd.read_parquet(fine_case["parquet"])


@pytest.fixture(scope="module")
def poly_ids(gdf):
    return list(gdf["poly_id"])


def test_centroid_max_eider_vs_rasterstats(fine_case, conn, poly_ids):
    """eider centroid MAX == rasterstats(all_touched=False) MAX, exactly."""
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_rasterstats,
    )

    e = run_eider(conn, fine_case["raster"], fine_case["parquet"], "centroid", "max")
    r = run_rasterstats(
        fine_case["raster"], fine_case["geojson"], "centroid", "max", poly_ids
    )
    rep = assert_agree(e, r, "eider", "rasterstats", TOL_MAX_EXACT)
    assert rep["agree"], rep


def test_all_touched_max_eider_vs_rasterstats(fine_case, conn, poly_ids):
    """eider all_touched MAX == rasterstats(all_touched=True) MAX, exactly."""
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_rasterstats,
    )

    e = run_eider(conn, fine_case["raster"], fine_case["parquet"], "all_touched", "max")
    r = run_rasterstats(
        fine_case["raster"], fine_case["geojson"], "all_touched", "max", poly_ids
    )
    rep = assert_agree(e, r, "eider", "rasterstats", TOL_MAX_EXACT)
    assert rep["agree"], rep


def test_all_touched_max_eider_vs_exactextract(fine_case, conn, gdf):
    """eider all_touched MAX == exactextract max (coverage>0 cells), exactly."""
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_exactextract,
    )

    e = run_eider(conn, fine_case["raster"], fine_case["parquet"], "all_touched", "max")
    x = run_exactextract(fine_case["raster"], gdf, "all_touched", "max")
    rep = assert_agree(e, x, "eider", "exactextract", TOL_MAX_EXACT)
    assert rep["agree"], rep


def test_all_touched_mean_eider_vs_rasterstats(fine_case, conn, poly_ids):
    """eider all_touched MEAN == rasterstats(all_touched=True) MEAN.

    Both average the SAME set of fully-counted touched cells, so they should
    match up to float accumulation order (looser absolute tolerance).
    """
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_rasterstats,
    )

    e = run_eider(
        conn, fine_case["raster"], fine_case["parquet"], "all_touched", "mean"
    )
    r = run_rasterstats(
        fine_case["raster"], fine_case["geojson"], "all_touched", "mean", poly_ids
    )
    rep = assert_agree(e, r, "eider", "rasterstats", TOL_ALL_TOUCHED_MEAN)
    assert rep["agree"], rep


def test_area_weighted_mean_eider_vs_exactextract(fine_case, conn, gdf):
    """eider area-weighted MEAN == exactextract coverage-weighted MEAN.

    The subtle pairing: eider weights each cell by ST_Intersection area of the
    cell box vs the polygon; exactextract uses its analytic coverage fraction.
    Different math, but should be very close (observed ~1e-8).
    """
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_exactextract,
    )

    e = run_eider(
        conn, fine_case["raster"], fine_case["parquet"], "area_weighted", "mean"
    )
    x = run_exactextract(fine_case["raster"], gdf, "area_weighted", "mean")
    rep = assert_agree(e, x, "eider", "exactextract", TOL_AREA_WEIGHTED_MEAN)
    assert rep["agree"], rep


def test_count_eider_vs_rasterstats(fine_case, conn, poly_ids):
    """eider touched-cell COUNT == rasterstats(all_touched=True) COUNT, exactly.

    Both are INTEGER cell counts. We do NOT compare exactextract count here: its
    "count" is the sum of coverage FRACTIONS (fractional area), a different
    definition that would not equal an integer cell count.
    """
    from scripts.bench_zonal_headtohead import (
        assert_agree,
        run_eider,
        run_rasterstats,
    )

    e = run_eider(
        conn, fine_case["raster"], fine_case["parquet"], "all_touched", "count"
    )
    r = run_rasterstats(
        fine_case["raster"], fine_case["geojson"], "all_touched", "count", poly_ids
    )
    rep = assert_agree(e, r, "eider", "rasterstats", TOL_COUNT_EXACT)
    assert rep["agree"], rep
