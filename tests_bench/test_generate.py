"""Tests for the synthetic COG + polygon data generator (Task 1)."""

import sys
from pathlib import Path

# Make the repo root importable so `scripts.bench_zonal_headtohead` resolves.
REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))


def test_generate_fine(tmp_path):
    from scripts.bench_zonal_headtohead import generate_data

    d = generate_data(tmp_path, "fine", 100, seed=1)
    import geopandas as gpd
    import rasterio

    with rasterio.open(d["raster"]) as r:
        assert r.count == 1
        assert r.crs.to_epsg() == 3857
        assert r.width == 2000
        # square pixels
        assert abs(r.transform.a) == abs(r.transform.e)

    g = gpd.read_parquet(d["parquet"])
    assert len(g) == 100
    assert "poly_id" in g.columns


def test_generate_coarse(tmp_path):
    from scripts.bench_zonal_headtohead import generate_data

    d = generate_data(tmp_path, "coarse", 50, seed=1)
    import geopandas as gpd
    import rasterio

    with rasterio.open(d["raster"]) as r:
        assert r.count == 1
        assert r.crs.to_epsg() == 3857
        assert r.width == 200
        assert r.height == 200

    g = gpd.read_parquet(d["parquet"])
    assert len(g) == 50
    assert "poly_id" in g.columns
    # sub-cell polygons: each should be far smaller than one pixel's area.
    with rasterio.open(d["raster"]) as r:
        pixel_area = abs(r.transform.a) * abs(r.transform.e)
    assert (g.geometry.area < pixel_area).all()
