"""Zonal-stats kernel head-to-head benchmark harness.

Task 1 implements the synthetic data generator: a tiled COG GeoTIFF plus
matching polygon sets (GeoParquet + GeoJSON) in a projected metric CRS with
square pixels, so that area weights are meaningful and consistent across the
contender tools (eider/DuckDB, exactextract, rasterstats).

Later tasks add the contender runners, correctness gate, and timing loop.
"""

from __future__ import annotations

import math
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


# ---------------------------------------------------------------------------
# Contender runners (Task 2)
#
# Each runner returns ``{poly_id: float}`` for a ``(raster, polys, convention,
# metric)`` request. Conventions: ``centroid``, ``all_touched``,
# ``area_weighted``. Metrics: ``max``, ``mean``, ``count``.
#
# Convention <-> tool mapping (kept honest, per the plan):
#   * centroid      -> rasterstats(all_touched=False) ; eider ST_Contains(centre)
#   * all_touched   -> rasterstats(all_touched=True), exactextract max,
#                      eider ST_Intersects(cell envelope)
#   * area_weighted -> exactextract mean (coverage-weighted),
#                      eider sum(v*area)/sum(area) over intersecting cells.
#                      rasterstats has no exact area weighting -> NotImplemented.
# ---------------------------------------------------------------------------

# read_geo column names for a local COG, discovered via
# ``DESCRIBE SELECT * FROM read_geo('grid.tif')`` (verified for EPSG:3857):
#   y (DOUBLE), x (DOUBLE), value (FLOAT)
# Coordinates are CELL CENTRES in the raster's own CRS (EPSG:3857 metres) --
# read_geo does NOT reproject to lon/lat. The polygons are generated in the
# same CRS, so the cell coords and polygon geometry are directly comparable.
READ_GEO_X = "x"
READ_GEO_Y = "y"
READ_GEO_VALUE = "value"

# Path to the loadable eider extension (relative to the repo root).
EIDER_EXTENSION_PATH = (
    Path(__file__).resolve().parents[1] / "target" / "debug" / "eider.duckdb_extension"
)

# Conventions and metrics as small enums-of-strings to avoid magic literals.
CONVENTION_CENTROID = "centroid"
CONVENTION_ALL_TOUCHED = "all_touched"
CONVENTION_AREA_WEIGHTED = "area_weighted"

METRIC_MAX = "max"
METRIC_MEAN = "mean"
METRIC_COUNT = "count"


def run_rasterstats(
    raster: str,
    geojson_path: str,
    convention: str,
    metric: str,
    poly_ids,
) -> dict:
    """rasterstats zonal_stats runner.

    ``all_touched=True`` for the all_touched convention, ``False`` (GDAL
    cell-centre rasterize) for centroid. rasterstats has no exact area
    weighting, so ``area_weighted`` raises ``NotImplementedError``.
    """
    from rasterstats import zonal_stats

    if convention == CONVENTION_AREA_WEIGHTED:
        raise NotImplementedError("rasterstats has no exact area weighting")

    all_touched = convention == CONVENTION_ALL_TOUCHED
    res = zonal_stats(
        geojson_path, raster, stats=[metric], all_touched=all_touched, nodata=None
    )
    return {
        pid: (r[metric] if r[metric] is not None else float("nan"))
        for pid, r in zip(poly_ids, res)
    }


def run_exactextract(raster: str, gdf, convention: str, metric: str) -> dict:
    """exactextract runner (coverage-weighted).

    exactextract weights every intersecting cell by its coverage fraction:
      * ``mean``  -> coverage-weighted mean  (used for area_weighted)
      * ``max``   -> max over all cells with coverage > 0 (used for all_touched)
      * ``count`` -> sum of coverage fractions

    Used for all_touched(max) and area_weighted(mean); NOT centroid (exactextract
    has no cell-centre-only mode). Output columns in 0.3.0 are named exactly by
    the op (``mean``/``max``/``count``), no ``band_`` prefix; it accepts a raster
    path plus a GeoDataFrame directly. Raster and gdf must share a CRS -- both
    are EPSG:3857 here.
    """
    from exactextract import exact_extract

    op = {METRIC_MAX: "max", METRIC_MEAN: "mean", METRIC_COUNT: "count"}[metric]
    df = exact_extract(raster, gdf, [op], output="pandas", include_cols=["poly_id"])
    value_col = next(c for c in df.columns if c != "poly_id")
    return {
        int(pid): float(val) for pid, val in zip(df["poly_id"], df[value_col])
    }


def eider_conn():
    """Open a DuckDB connection with the eider extension + spatial loaded."""
    import duckdb

    conn = duckdb.connect(config={"allow_unsigned_extensions": True})
    conn.execute(f"LOAD '{EIDER_EXTENSION_PATH}'")
    conn.execute("INSTALL spatial; LOAD spatial;")
    return conn


def _poly_bbox(conn, polys_parquet: str) -> tuple[float, float, float, float]:
    """Bounding box (xmin, ymin, xmax, ymax) of all polygons, in the polys' CRS."""
    row = conn.execute(
        f"""
        SELECT ST_XMin(ext), ST_YMin(ext), ST_XMax(ext), ST_YMax(ext)
        FROM (SELECT ST_Extent_Agg(geometry) AS ext
              FROM read_parquet('{polys_parquet}'))
        """
    ).fetchone()
    return tuple(float(v) for v in row)  # type: ignore[return-value]


# Number of cells of padding applied to the polygon bbox when slicing the field.
# read_geo's lat/lon bbox pushdown only fires for EPSG:4326 COGs (dim names
# lat/lon); for EPSG:3857 the dims are y/x with no matching pushdown param, so we
# prune with a SQL WHERE on x/y instead. A generous pad keeps every touched cell.
_BBOX_PAD_CELLS = 2.0
# Approximate cell size used only to pad the bbox slice; the exact dx/dy used in
# the geometry predicates is derived from the read itself.
_APPROX_CELL_M = PIXEL_SIZE_M


def _field_cte(raster: str, bbox: tuple[float, float, float, float]) -> str:
    """A `field` CTE selecting cell-CENTRE (x, y, value) within a padded bbox.

    The WHERE on x/y prunes the read to the polygons' neighbourhood (read_geo's
    lat/lon bbox pushdown does not apply to EPSG:3857 COGs, whose dims are y/x).

    Half-pixel correction: ``read_geo`` reports each cell's UPPER-LEFT CORNER,
    not its centre (verified empirically: a 30 m grid with origin x0=0 yields
    read_geo x = 0, 30, 60 ... whereas the true cell centres are 15, 45, 75 ...
    = x + dx/2; and read_geo y = -1470, -1440 ... whereas the true centres are
    -1485, -1455 ... = y - dy/2, since y is the top edge and the cell extends
    downward). All downstream predicates (ST_Point centroid test, the cell-box
    envelope, the index-join snap) assume CENTRES, so we shift the raw corner
    coords to centres here. Without this shift the cell boxes are offset by half
    a pixel and MAX/centroid selection picks the wrong cell.
    """
    xmin, ymin, xmax, ymax = bbox
    pad = _BBOX_PAD_CELLS * _APPROX_CELL_M
    return f"""
        raw AS (
            SELECT {READ_GEO_X} AS x, {READ_GEO_Y} AS y, {READ_GEO_VALUE} AS v
            FROM read_geo('{raster}')
            WHERE {READ_GEO_X} BETWEEN {xmin - pad} AND {xmax + pad}
              AND {READ_GEO_Y} BETWEEN {ymin - pad} AND {ymax + pad}
              -- Drop nodata cells so count/area-weighted means match the
              -- coverage-based tools (exactextract/rasterstats ignore nodata).
              AND {READ_GEO_VALUE} IS NOT NULL
        ),
        step AS (
            SELECT (max(x) - min(x)) / nullif(count(DISTINCT x) - 1, 0) AS dx,
                   (max(y) - min(y)) / nullif(count(DISTINCT y) - 1, 0) AS dy
            FROM raw
        ),
        field AS (
            -- Shift read_geo corner coords to true cell centres (see docstring).
            SELECT raw.x + s.dx / 2 AS x,
                   raw.y - s.dy / 2 AS y,
                   raw.v AS v
            FROM raw CROSS JOIN step s
        )
    """


def _metric_agg(metric: str) -> str:
    return {
        METRIC_MAX: "max(z.v)",
        METRIC_MEAN: "avg(z.v)",
        METRIC_COUNT: "count(*)",
    }[metric]


def run_eider(
    conn, raster: str, polys_parquet: str, convention: str, metric: str
) -> dict:
    """eider/DuckDB + spatial runner: per-convention spatial-join zonal stats.

    Cell coords from read_geo are cell centres in the raster CRS (EPSG:3857
    metres); polygons are read from GeoParquet in the same CRS, so the join is
    CRS-consistent without any reprojection. dx/dy are derived from the read.
    """
    bbox = _poly_bbox(conn, polys_parquet)
    field = _field_cte(raster, bbox)

    if convention == CONVENTION_CENTROID:
        sql = f"""
            WITH {field}
            SELECT v.poly_id, {_metric_agg(metric)} AS metric
            FROM read_parquet('{polys_parquet}') v
            JOIN field z
              ON ST_Contains(v.geometry, ST_Point(z.x, z.y))
            GROUP BY v.poly_id
        """
    elif convention == CONVENTION_ALL_TOUCHED:
        sql = f"""
            WITH {field}
            SELECT v.poly_id, {_metric_agg(metric)} AS metric
            FROM read_parquet('{polys_parquet}') v
            CROSS JOIN step s
            JOIN field z
              ON ST_Intersects(
                   v.geometry,
                   ST_MakeEnvelope(z.x - s.dx / 2, z.y - s.dy / 2,
                                   z.x + s.dx / 2, z.y + s.dy / 2))
            GROUP BY v.poly_id
        """
    elif convention == CONVENTION_AREA_WEIGHTED:
        if metric != METRIC_MEAN:
            raise NotImplementedError(
                "area_weighted is only defined for the mean metric"
            )
        sql = f"""
            WITH {field}
            SELECT v.poly_id,
                   sum(z.v * ST_Area(ST_Intersection(v.geometry, cell.box)))
                   / nullif(sum(ST_Area(ST_Intersection(v.geometry, cell.box))), 0)
                     AS metric
            FROM read_parquet('{polys_parquet}') v
            CROSS JOIN step s
            JOIN field z
              ON ST_Intersects(
                   v.geometry,
                   ST_MakeEnvelope(z.x - s.dx / 2, z.y - s.dy / 2,
                                   z.x + s.dx / 2, z.y + s.dy / 2)),
            LATERAL (SELECT ST_MakeEnvelope(z.x - s.dx / 2, z.y - s.dy / 2,
                                            z.x + s.dx / 2, z.y + s.dy / 2)
                            AS box) cell
            GROUP BY v.poly_id
        """
    else:
        raise ValueError(f"unknown convention {convention!r}")

    rows = conn.execute(sql).fetchall()
    return {int(pid): float(val) for pid, val in rows if val is not None}


def run_eider_indexjoin(
    conn, raster: str, polys_parquet: str, convention: str, metric: str
) -> dict:
    """eider point-model runner via an arithmetic cell-index equi-join.

    For Regime 1 (sub-cell polygons) we model each polygon by its centroid and
    snap it to the nearest cell index using the read's origin/step, then equi-join
    on integer (ix, iy). This avoids the geometry predicate entirely. Valid only
    for the sub-cell / point-model case; labelled as such in the results.
    """
    bbox = _poly_bbox(conn, polys_parquet)
    field = _field_cte(raster, bbox)

    sql = f"""
        WITH {field},
        origin AS (SELECT min(x) AS x0, min(y) AS y0 FROM field),
        cells AS (
            SELECT round((z.x - o.x0) / s.dx) AS ix,
                   round((z.y - o.y0) / s.dy) AS iy,
                   z.v AS v
            FROM field z CROSS JOIN step s CROSS JOIN origin o
        ),
        centroids AS (
            SELECT v.poly_id,
                   round((ST_X(ST_Centroid(v.geometry)) - o.x0) / s.dx) AS ix,
                   round((ST_Y(ST_Centroid(v.geometry)) - o.y0) / s.dy) AS iy
            FROM read_parquet('{polys_parquet}') v
            CROSS JOIN step s CROSS JOIN origin o
        )
        SELECT c.poly_id, {_metric_agg_index(metric)} AS metric
        FROM centroids c
        JOIN cells z ON c.ix = z.ix AND c.iy = z.iy
        GROUP BY c.poly_id
    """
    rows = conn.execute(sql).fetchall()
    return {int(pid): float(val) for pid, val in rows if val is not None}


def _metric_agg_index(metric: str) -> str:
    return {
        METRIC_MAX: "max(z.v)",
        METRIC_MEAN: "avg(z.v)",
        METRIC_COUNT: "count(*)",
    }[metric]


# ---------------------------------------------------------------------------
# Correctness gate (Task 3)
# ---------------------------------------------------------------------------
def assert_agree(
    a: dict,
    b: dict,
    name_a: str,
    name_b: str,
    abs_tol: float,
) -> dict:
    """Compare two ``{poly_id: float}`` results on their overlapping poly_ids.

    Comparison is NaN-aware:
      * both values NaN  -> agreement (both tools declined the same polygon);
      * exactly one NaN  -> mismatch (the tools disagree about coverage);
      * both finite       -> mismatch iff ``abs(a - b) > abs_tol``.

    Parameters
    ----------
    a, b:
        Result dicts keyed by ``poly_id``. Only ids present in BOTH are compared.
    name_a, name_b:
        Labels used in the no-overlap assertion message.
    abs_tol:
        Absolute tolerance for finite comparisons.

    Returns
    -------
    dict with keys ``agree`` (bool), ``max_abs_diff`` (float over comparable
    finite pairs), ``n_compared`` (int), ``n_mismatch`` (int) and ``examples``
    (up to five ``(poly_id, a_value, b_value)`` mismatch tuples).
    """
    ids = set(a) & set(b)
    assert ids, f"{name_a}/{name_b}: no overlapping poly_ids"

    mismatches: list[tuple] = []
    max_abs_diff = 0.0
    for i in sorted(ids):
        av, bv = a[i], b[i]
        a_nan, b_nan = math.isnan(av), math.isnan(bv)
        if a_nan and b_nan:
            continue  # both declined -> agreement
        if a_nan or b_nan:
            mismatches.append((i, av, bv))  # exactly one NaN -> disagreement
            continue
        diff = abs(av - bv)
        if diff > max_abs_diff:
            max_abs_diff = diff
        if diff > abs_tol:
            mismatches.append((i, av, bv))

    return {
        "agree": not mismatches,
        "max_abs_diff": max_abs_diff,
        "n_compared": len(ids),
        "n_mismatch": len(mismatches),
        "examples": mismatches[:5],
    }


def main() -> None:
    """Placeholder entry point; the full harness arrives in later tasks."""
    raise SystemExit(
        "bench_zonal_headtohead: data generator only (Task 1). "
        "Runners, correctness gate, and timing harness land in later tasks."
    )


if __name__ == "__main__":
    main()
