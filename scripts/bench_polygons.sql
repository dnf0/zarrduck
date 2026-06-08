-- bench_polygons.sql — zonal-stats query patterns over a grid, with correctness + cost.
--
-- Run:  duckdb -unsigned < scripts/bench_polygons.sql
--   (requires the `spatial` extension; the join benchmarks below use SYNTHETIC
--    in-memory grids so no eider extension or Zarr file is needed — see the
--    commented eider-read pattern at the bottom for the real pruned-read step.)
--
-- These are single-machine, synthetic-data numbers; absolute times vary by
-- hardware and the field is spatially-uncorrelated random (so MEAN differences
-- between conventions are understated vs real, gradient hazard surfaces). The
-- robust findings are the ORDERING and the CORRECTNESS gaps, not the seconds.

LOAD spatial;

-- ============================================================================
-- REGIME 1 — point-sample: asset is much SMALLER than a cell (coarse grid).
-- e.g. a few-hundred-metre asset over a km-scale climate grid → it sits in ONE
-- cell, so you want that cell's value. Model the asset as a point/centroid.
-- ============================================================================
CREATE TABLE coarse AS  -- 144 x 73 = 10,512 cells (2.5deg-ish), value per cell
  SELECT gi.i AS i, gj.j AS j, (random()*50) AS v FROM generate_series(0,72) gi(i), generate_series(0,143) gj(j);
CREATE TABLE pts AS     -- 1,000,000 asset centroids in index space
  SELECT id, (random()*143) AS x, (random()*72) AS y FROM range(0,1000000) t(id);
.timer on
SELECT '[R1] point-sample, arithmetic index equi-join (1M assets)' AS bench;
SELECT count(*) matched, round(avg(c.v),3) FROM pts p JOIN coarse c
  ON CAST(round(p.y) AS INT)=c.i AND CAST(round(p.x) AS INT)=c.j;          -- fastest, exact for point model
SELECT '[R1] point-sample, spatial ST_Contains (1M assets)' AS bench;
SELECT count(*) matched, round(avg(c.v),3) FROM pts p JOIN coarse c
  ON ST_Contains(ST_MakeEnvelope(c.j-0.5,c.i-0.5,c.j+0.5,c.i+0.5), ST_Point(p.x,p.y)); -- correct too, ~30x slower

-- ============================================================================
-- REGIME 2 — zonal: asset SPANS many cells (fine grid, e.g. 30m hazard).
-- A ~300m footprint over a 30m grid ≈ 100 cells → real per-asset aggregation.
-- Footprints here are DIAMONDS (rotated squares) so bbox != footprint, which
-- exposes the correctness of bbox-based shortcuts.
-- ============================================================================
CREATE TABLE field AS   -- 2000 x 2000 = 4M cells; cell (i,j) center = (j+0.5, i+0.5)
  SELECT gi.i AS i, gj.j AS j, (random()*50) AS v,
         ST_Point(gj.j+0.5, gi.i+0.5) AS pt,
         ST_MakeEnvelope(gj.j, gi.i, gj.j+1, gi.i+1) AS box
  FROM generate_series(0,1999) gi(i), generate_series(0,1999) gj(j);
CREATE TABLE assets AS  -- 100k diamond footprints, radius ~5 cells (~300m at 30m)
  SELECT id, (10+random()*1980) AS cx, (10+random()*1980) AS cy, 5.0 AS r FROM range(0,100000) t(id);
ALTER TABLE assets ADD COLUMN geom GEOMETRY;
UPDATE assets SET geom = ST_GeomFromText(
  'POLYGON((' || (cx-r)||' '||cy||', '||cx||' '||(cy+r)||', '||(cx+r)||' '||cy||', '||cx||' '||(cy-r)||', '||(cx-r)||' '||cy||'))');

-- --- Correct options (pick the CONVENTION that matches your definition) ---
SELECT '[R2] centroid membership: ST_Contains(asset, cell_center)' AS bench;   -- exact; cheapest; may drop edge cells
SELECT round(avg(mx),4) mean_MAX, round(avg(mn),4) mean_MEAN FROM (
  SELECT a.id, max(f.v) mx, avg(f.v) mn FROM assets a JOIN field f ON ST_Contains(a.geom, f.pt) GROUP BY a.id);
SELECT '[R2] all-touched: ST_Intersects(asset, cell_box)' AS bench;            -- exact; conservative (best for MAX exposure)
SELECT round(avg(mx),4) mean_MAX, round(avg(mn),4) mean_MEAN FROM (
  SELECT a.id, max(f.v) mx, avg(f.v) mn FROM assets a JOIN field f ON ST_Intersects(a.geom, f.box) GROUP BY a.id);
SELECT '[R2] area-weighted MEAN: ST_Intersection area' AS bench;               -- exact area-true mean; ~6x cost
SELECT round(avg(wmean),4) area_weighted_MEAN FROM (
  SELECT a.id, sum(f.v*ST_Area(ST_Intersection(a.geom,f.box)))/sum(ST_Area(ST_Intersection(a.geom,f.box))) wmean
  FROM assets a JOIN field f ON ST_Intersects(a.geom, f.box) GROUP BY a.id);

-- --- Anti-patterns (shown so they are not mistaken for correct/fast) ---
SELECT '[R2] WRONG: raw bbox index-block, no spatial refine' AS bench;         -- over-includes corner cells: MAX inflated
WITH cand AS (SELECT a.id, gi.i, gj.j FROM assets a,
   generate_series(CAST(floor(a.cy-a.r) AS INT),CAST(ceil(a.cy+a.r) AS INT)) gi(i),
   generate_series(CAST(floor(a.cx-a.r) AS INT),CAST(ceil(a.cx+a.r) AS INT)) gj(j))
SELECT round(avg(mx),4) mean_MAX_INFLATED FROM (
  SELECT c.id, max(f.v) mx FROM cand c JOIN field f ON f.i=c.i AND f.j=c.j GROUP BY c.id);
-- NB: only exact when the footprint IS an axis-aligned rectangle. For arbitrary
-- shapes use the spatial conventions above. (Also avoid two-sided BETWEEN
-- range-joins entirely — they degrade to a nested/IEJoin and are ~1000x slower.)

-- ============================================================================
-- The eider step (the part eider actually owns): instead of synthesising
-- `field`, read ONLY the Zarr chunks intersecting your assets' extent, then run
-- the join above on that small slice. Requires the eider extension:
--   LOAD '/absolute/path/to/eider.duckdb_extension';
--   SET VARIABLE bbox = (SELECT ST_Extent_Agg(geom) FROM ST_Read('assets.geojson'));
--   CREATE TABLE field AS SELECT lat, lon, value FROM read_geo('hazard.zarr/depth',
--     lon_min := ST_XMin(getvariable('bbox')), lat_min := ST_YMin(getvariable('bbox')),
--     lon_max := ST_XMax(getvariable('bbox')), lat_max := ST_YMax(getvariable('bbox')));
-- ============================================================================
