"""Zonal-stats kernel head-to-head benchmark harness.

Task 1 implements the synthetic data generator: a tiled COG GeoTIFF plus
matching polygon sets (GeoParquet + GeoJSON) in a projected metric CRS with
square pixels, so that area weights are meaningful and consistent across the
contender tools (eider/DuckDB, exactextract, rasterstats).

Later tasks add the contender runners, correctness gate, and timing loop.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import geopandas as gpd
import numpy as np
import rasterio
from rasterio.transform import Affine
from shapely.geometry import Polygon

# --- Grid geometry constants (projected metric CRS, square pixels) ---------
RASTER_CRS = "EPSG:3857"
PIXEL_SIZE_M = 30.0  # square pixel side length in CRS (metre) units
GRID_ORIGIN_X = 0.0  # x of the upper-left corner of the raster
GRID_ORIGIN_Y = 0.0  # y of the upper-left corner of the raster

# Default raster shapes (rows, cols) per regime.
FINE_SHAPE = (2000, 2000)
COARSE_SHAPE = (200, 200)

# Gradient field range used to build the smooth base surface.
GRADIENT_MAX = 50.0
GRADIENT_NOISE_STD = 2.0

# Diamond footprint radius for the "fine" regime, in pixels.
FINE_DIAMOND_RADIUS_PX = 5.0
# Sub-cell square side for the "coarse" regime, in pixels.
COARSE_SQUARE_SIDE_PX = 0.3


@dataclass(frozen=True)
class Regime:
    """Configuration for a synthetic data-generation regime."""

    name: str
    shape: tuple[int, int]


REGIMES: dict[str, Regime] = {
    "fine": Regime(name="fine", shape=FINE_SHAPE),
    "coarse": Regime(name="coarse", shape=COARSE_SHAPE),
}


def _build_field(shape: tuple[int, int], rng: np.random.Generator) -> np.ndarray:
    """Smooth diagonal gradient plus mild Gaussian noise, as float32."""
    height, width = shape
    base = np.add.outer(
        np.linspace(0.0, GRADIENT_MAX, height),
        np.linspace(0.0, GRADIENT_MAX, width),
    )
    field = base + rng.normal(0.0, GRADIENT_NOISE_STD, size=shape)
    return field.astype(np.float32)


def _transform() -> Affine:
    """Square-pixel north-up affine transform for the projected grid."""
    return Affine(PIXEL_SIZE_M, 0.0, GRID_ORIGIN_X, 0.0, -PIXEL_SIZE_M, GRID_ORIGIN_Y)


def _write_cog(path: Path, field: np.ndarray, transform: Affine) -> None:
    """Write a single-band float32 tiled GeoTIFF that rasterio can reopen as a COG."""
    height, width = field.shape
    profile = {
        "driver": "GTiff",
        "dtype": "float32",
        "count": 1,
        "height": height,
        "width": width,
        "crs": RASTER_CRS,
        "transform": transform,
        "tiled": True,
        "blockxsize": 256,
        "blockysize": 256,
        "compress": "deflate",
    }
    with rasterio.open(path, "w", **profile) as dst:
        dst.write(field, 1)


def _diamond(cx: float, cy: float, radius: float) -> Polygon:
    """Diamond (rotated square) footprint centred at (cx, cy)."""
    return Polygon(
        [
            (cx - radius, cy),
            (cx, cy + radius),
            (cx + radius, cy),
            (cx, cy - radius),
        ]
    )


def _square(cx: float, cy: float, half: float) -> Polygon:
    """Axis-aligned square footprint centred at (cx, cy) with the given half-side."""
    return Polygon(
        [
            (cx - half, cy - half),
            (cx - half, cy + half),
            (cx + half, cy + half),
            (cx + half, cy - half),
        ]
    )


def _random_centers(
    n: int,
    transform: Affine,
    shape: tuple[int, int],
    margin: float,
    rng: np.random.Generator,
) -> tuple[np.ndarray, np.ndarray]:
    """Random (x, y) centres well inside the raster extent, avoiding edges by `margin`."""
    height, width = shape
    x_min = transform.c
    x_max = transform.c + width * transform.a
    y_max = transform.f
    y_min = transform.f + height * transform.e  # transform.e is negative (north-up)

    cx = rng.uniform(x_min + margin, x_max - margin, size=n)
    cy = rng.uniform(y_min + margin, y_max - margin, size=n)
    return cx, cy


def generate_data(
    out_dir,
    regime: str,
    n_polys: int,
    seed: int = 42,
    shape: tuple[int, int] | None = None,
) -> dict:
    """Generate a synthetic tiled COG and a matching polygon set.

    Parameters
    ----------
    out_dir:
        Directory to write ``grid.tif``, ``polys.parquet`` and ``polys.geojson`` into.
    regime:
        ``"fine"`` (R2) -> large smooth grid + diamond footprints (~5 px radius);
        ``"coarse"`` (R1) -> small grid + sub-cell square footprints.
    n_polys:
        Number of polygons to generate.
    seed:
        Seed for the numpy RNG (determinism).
    shape:
        Optional ``(height, width)`` override for the raster. Defaults to the
        regime's standard shape.

    Returns
    -------
    dict with keys ``raster``, ``parquet``, ``geojson``, ``transform``, ``crs``,
    ``shape``.
    """
    if regime not in REGIMES:
        raise ValueError(f"unknown regime {regime!r}; expected one of {sorted(REGIMES)}")

    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    grid_shape = shape if shape is not None else REGIMES[regime].shape
    rng = np.random.default_rng(seed)

    field = _build_field(grid_shape, rng)
    transform = _transform()

    raster_path = out_dir / "grid.tif"
    _write_cog(raster_path, field, transform)

    if regime == "fine":
        radius = FINE_DIAMOND_RADIUS_PX * PIXEL_SIZE_M
        cx, cy = _random_centers(n_polys, transform, grid_shape, margin=radius * 2, rng=rng)
        geometries = [_diamond(x, y, radius) for x, y in zip(cx, cy)]
    else:  # coarse
        half = (COARSE_SQUARE_SIDE_PX * PIXEL_SIZE_M) / 2.0
        cx, cy = _random_centers(
            n_polys, transform, grid_shape, margin=PIXEL_SIZE_M * 2, rng=rng
        )
        geometries = [_square(x, y, half) for x, y in zip(cx, cy)]

    gdf = gpd.GeoDataFrame(
        {"poly_id": range(n_polys)},
        geometry=geometries,
        crs=RASTER_CRS,
    )

    parquet_path = out_dir / "polys.parquet"
    geojson_path = out_dir / "polys.geojson"
    gdf.to_parquet(parquet_path)
    gdf.to_file(geojson_path, driver="GeoJSON")

    return {
        "raster": str(raster_path),
        "parquet": str(parquet_path),
        "geojson": str(geojson_path),
        "transform": transform,
        "crs": RASTER_CRS,
        "shape": grid_shape,
    }


def main() -> None:
    """Placeholder entry point; the full harness arrives in later tasks."""
    raise SystemExit(
        "bench_zonal_headtohead: data generator only (Task 1). "
        "Runners, correctness gate, and timing harness land in later tasks."
    )


if __name__ == "__main__":
    main()
