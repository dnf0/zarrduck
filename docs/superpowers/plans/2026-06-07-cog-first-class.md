# First-Class COG Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `read_geo('*.tif')` / `read_zarr_metadata('*.tif')` return georeferenced, type-correct results by fixing the COG metadata `VirtualCogStore` synthesizes — with no change to the SQL dispatch.

**Architecture:** Extend the hand-written TIFF/IFD parser in `geozarr_core/src/cog.rs` to read dtype, fill value, compression, band count, the GeoTIFF affine, and CRS. `VirtualCogStore` then synthesizes a `.zarray` (real dtype/fill) plus a `.zattrs` carrying `_ARRAY_DIMENSIONS` and a `geozarr` block (affine `spatial_transform` + `crs`), so the **existing** affine-coordinate, CRS-reporting, and bbox-pruning machinery works unchanged. Deflate tiles are inflated in the store layer before `zarrs` decodes.

**Tech Stack:** Rust (`geozarr_core`), `zarrs`, `flate2` (zlib), criterion-free unit tests with hand-built TIFF byte buffers, a committed rasterio-generated GeoTIFF fixture, Docusaurus docs.

---

## Conventions & prerequisites

Repo root `/Users/danielfisher/repos/zarrduck`, branch `feat/cog-first-class` (already created off `main`). TDD: failing test → run → implement → pass → commit. Conventional Commits; every commit `--no-gpg-sign` ending with:
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`

Pre-commit hook runs fmt/clippy/whitespace; if it blocks, fix and re-commit. The working tree may have unrelated modified/untracked files — **never `git add -A`**; stage only the files each task names.

Test commands:
- Unit (one module): `cargo test -p geozarr_core cog::` / `virtual_store::`
- Full workspace: `cargo test` then `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`
- Docs build: `cd docs && (test -d node_modules || npm ci) && npm run build`

### Key facts established from source (do not re-derive)
- `read_geo` binds bounds by dimension name: `format!("{}_min", name)` / `_max` (`extension/src/table_function.rs:117-128`). Registered params are `lat_min/lat_max/lon_min/lon_max` (+ time). → **Naming the two COG dims `lat`/`lon` makes bbox pushdown work** through the affine branch of `compute_bounds` (`dataset.rs:190-229`), which fires when `coords` is empty (the COG case).
- `ZarrDataset` reads `spatial_transform` and CRS from the array's attributes via `parse_geozarr_metadata` (`dataset.rs:42-55`, `metadata.rs`), and dimension names from `_ARRAY_DIMENSIONS` (`dataset.rs:243`). The relevant structs:
  ```rust
  // geozarr_core/src/metadata.rs
  pub struct SpatialTransform { pub scale: Vec<f64>, pub translation: Vec<f64> }
  // geozarr block: { "geozarr": { "crs": "EPSG:n", "spatial_transform": { "scale": [...], "translation": [...] } } }
  ```
- `apply_transform`: `value = translation + grid_index * scale` (`coordinates.rs:4-8`). For a north-up GeoTIFF, dim 0 (rows→lat) uses `scale = -pixelScaleY`, `translation = tiepointY`; dim 1 (cols→lon) uses `scale = +pixelScaleX`, `translation = tiepointX`.
- COGs reach `VirtualCogStore` today (the STAC short-circuit doesn't match `.tif`); **do not touch the dispatch/short-circuit**.

### Current `CogMetadata` (baseline to extend)
```rust
pub struct CogMetadata {
    pub image_width: u32, pub image_length: u32,
    pub tile_width: u32, pub tile_length: u32,
    pub tile_offsets: Vec<u64>, pub tile_byte_counts: Vec<u64>,
}
```

## File structure
- Modify: `geozarr_core/src/cog.rs` (parser + metadata; Tasks 1–4)
- Modify: `geozarr_core/src/virtual_store.rs` (synthesis + Deflate decode; Tasks 5–6)
- Modify: `geozarr_core/Cargo.toml` (add `flate2`; Task 6)
- Create: `scripts/generate_cog_fixture.py`; Create fixtures under `geozarr_core/tests/fixtures/` (Task 7)
- Create: `geozarr_core/tests/cog_e2e.rs` (Task 8)
- Modify: `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md` (Task 9)
- Verify only: CI + scope (Task 10)

---

## Task 1: Parse scalar TIFF tags (dtype/band/compression inputs)

**Files:** Modify `geozarr_core/src/cog.rs`.

- [ ] **Step 1: Extend `CogMetadata` and capture endianness**

Add fields (keep existing ones):
```rust
#[derive(Debug, Default, Clone)]
pub struct CogMetadata {
    pub image_width: u32,
    pub image_length: u32,
    pub tile_width: u32,
    pub tile_length: u32,
    pub tile_offsets: Vec<u64>,
    pub tile_byte_counts: Vec<u64>,
    pub is_little_endian: bool,
    pub bits_per_sample: u16,   // 258; default filled in Step 3
    pub sample_format: u16,     // 339; 1=uint (default), 2=int, 3=float
    pub samples_per_pixel: u16, // 277; default 1
    pub compression: u16,       // 259; 1=none (default)
    pub predictor: u16,         // 317; 1=none (default)
}
```

- [ ] **Step 2: Write the failing test**

Add to the `tests` module in `cog.rs`:
```rust
#[test]
fn test_parse_scalar_tags() {
    // II, magic 42, IFD at 8; 6 entries: width,length,tilew,tilel,bits,sampfmt
    let mut b = vec![0u8; 100];
    b[0..2].copy_from_slice(b"II");
    b[2..4].copy_from_slice(&42u16.to_le_bytes());
    b[4..8].copy_from_slice(&8u32.to_le_bytes());
    b[8..10].copy_from_slice(&6u16.to_le_bytes()); // 6 entries
    let mut o = 10;
    let mut put = |b: &mut [u8], o: usize, tag: u16, typ: u16, val: u32| {
        b[o..o + 2].copy_from_slice(&tag.to_le_bytes());
        b[o + 2..o + 4].copy_from_slice(&typ.to_le_bytes());
        b[o + 4..o + 8].copy_from_slice(&1u32.to_le_bytes());
        b[o + 8..o + 12].copy_from_slice(&val.to_le_bytes());
    };
    put(&mut b, o, 256, 4, 4); o += 12;        // ImageWidth=4
    put(&mut b, o, 257, 4, 2); o += 12;        // ImageLength=2
    put(&mut b, o, 322, 3, 4); o += 12;        // TileWidth=4 (SHORT)
    put(&mut b, o, 323, 3, 2); o += 12;        // TileLength=2
    put(&mut b, o, 258, 3, 16); o += 12;       // BitsPerSample=16
    put(&mut b, o, 339, 3, 2);                 // SampleFormat=2 (signed int)
    let m = parse_cog_metadata(&b).unwrap();
    assert!(m.is_little_endian);
    assert_eq!(m.bits_per_sample, 16);
    assert_eq!(m.sample_format, 2);
    assert_eq!(m.samples_per_pixel, 1); // defaulted
    assert_eq!(m.compression, 1);       // defaulted
}
```

- [ ] **Step 3: Run it (fails to compile / fails assert)**

Run: `cargo test -p geozarr_core cog::tests::test_parse_scalar_tags`
Expected: FAIL (new fields unset / defaults not applied).

- [ ] **Step 4: Implement parsing + defaults**

In `parse_cog_metadata`, set `meta.is_little_endian = header.is_little_endian;` right after creating `meta`. Add match arms (alongside the existing 256/257/322/323/324/325):
```rust
258 => meta.bits_per_sample = extract_single_val() as u16,
277 => meta.samples_per_pixel = extract_single_val() as u16,
339 => meta.sample_format = extract_single_val() as u16,
259 => meta.compression = extract_single_val() as u16,
317 => meta.predictor = extract_single_val() as u16,
```
After the entry loop, apply TIFF defaults for absent tags:
```rust
if meta.bits_per_sample == 0 { meta.bits_per_sample = 32; }
if meta.sample_format == 0 { meta.sample_format = 1; }   // unsigned int
if meta.samples_per_pixel == 0 { meta.samples_per_pixel = 1; }
if meta.compression == 0 { meta.compression = 1; }       // none
if meta.predictor == 0 { meta.predictor = 1; }           // none
```

- [ ] **Step 5: Run to pass**

Run: `cargo test -p geozarr_core cog::tests::test_parse_scalar_tags`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add geozarr_core/src/cog.rs
git commit --no-gpg-sign -m "feat(cog): parse bits/sample-format/bands/compression/predictor tags

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: dtype mapping + band guard

**Files:** Modify `geozarr_core/src/cog.rs`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_zarr_dtype_and_band_guard() {
    let mut m = CogMetadata { is_little_endian: true, samples_per_pixel: 1, ..Default::default() };
    m.bits_per_sample = 16; m.sample_format = 2;
    assert_eq!(m.zarr_dtype().unwrap(), "<i2");
    m.bits_per_sample = 32; m.sample_format = 3;
    assert_eq!(m.zarr_dtype().unwrap(), "<f4");
    m.bits_per_sample = 8; m.sample_format = 1;
    assert_eq!(m.zarr_dtype().unwrap(), "|u1");
    // big-endian flips the prefix
    m.is_little_endian = false; m.bits_per_sample = 16; m.sample_format = 1;
    assert_eq!(m.zarr_dtype().unwrap(), ">u2");
    // multi-band is rejected
    m.samples_per_pixel = 3;
    assert!(m.zarr_dtype().is_err());
    // unsupported bit depth rejected
    let bad = CogMetadata { samples_per_pixel: 1, bits_per_sample: 12, sample_format: 1, ..Default::default() };
    assert!(bad.zarr_dtype().is_err());
}
```

- [ ] **Step 2: Run (fail — method missing)**

Run: `cargo test -p geozarr_core cog::tests::test_zarr_dtype_and_band_guard`
Expected: FAIL (no `zarr_dtype`).

- [ ] **Step 3: Implement `zarr_dtype`**

Add to `impl CogMetadata` in `cog.rs`:
```rust
impl CogMetadata {
    /// Numpy/Zarr-V2 dtype string for this COG's single band, e.g. "<i2".
    /// Errors on multi-band or unsupported bit-depth/sample-format combinations.
    pub fn zarr_dtype(&self) -> Result<String, String> {
        if self.samples_per_pixel != 1 {
            return Err(format!(
                "multi-band COGs not yet supported (SamplesPerPixel={})",
                self.samples_per_pixel
            ));
        }
        let endian = if self.bits_per_sample <= 8 {
            "|"
        } else if self.is_little_endian {
            "<"
        } else {
            ">"
        };
        let kind = match self.sample_format {
            3 => "f",          // float
            2 => "i",          // signed int
            1 => "u",          // unsigned int
            other => return Err(format!("unsupported TIFF SampleFormat {other}")),
        };
        let bytes = match self.bits_per_sample {
            8 => 1, 16 => 2, 32 => 4, 64 => 8,
            other => return Err(format!("unsupported BitsPerSample {other}")),
        };
        if kind == "f" && bytes < 4 {
            return Err(format!("unsupported float width {} bits", self.bits_per_sample));
        }
        Ok(format!("{endian}{kind}{bytes}"))
    }
}
```

- [ ] **Step 4: Run to pass**; Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add geozarr_core/src/cog.rs
git commit --no-gpg-sign -m "feat(cog): map TIFF dtype to zarr dtype; reject multi-band/unsupported

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Compression enum, predictor guard, nodata

**Files:** Modify `geozarr_core/src/cog.rs`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_compression_and_predictor_and_nodata() {
    use super::CogCompression::*;
    let mut m = CogMetadata { compression: 1, predictor: 1, ..Default::default() };
    assert!(matches!(m.compression_kind(), Ok(None)));
    m.compression = 8;
    assert!(matches!(m.compression_kind(), Ok(Deflate)));
    m.compression = 32946; // old-style deflate
    assert!(matches!(m.compression_kind(), Ok(Deflate)));
    m.compression = 5; // LZW
    assert!(m.compression_kind().is_err());
    // predictor != 1 with deflate is rejected
    m.compression = 8; m.predictor = 2;
    assert!(m.compression_kind().is_err());
    // nodata parse
    assert_eq!(CogMetadata::parse_nodata("  -9999  "), Some(-9999.0));
    assert_eq!(CogMetadata::parse_nodata("nan"), None);
}
```

- [ ] **Step 2: Run (fail)**; Expected: FAIL (no `CogCompression` / methods).

- [ ] **Step 3: Implement compression enum + nodata + GDAL_NODATA tag parse**

Add near the top of `cog.rs`:
```rust
#[derive(Debug, PartialEq)]
pub enum CogCompression { None, Deflate }
```
Add a `nodata: Option<f64>` field to `CogMetadata` (Task 1 struct). Add methods:
```rust
impl CogMetadata {
    /// Resolve the TIFF Compression+Predictor tags to a supported kind, or error.
    pub fn compression_kind(&self) -> Result<CogCompression, String> {
        let comp = match self.compression {
            1 => CogCompression::None,
            8 | 32946 => CogCompression::Deflate,
            other => return Err(format!("unsupported COG compression {other} (only uncompressed and Deflate are supported)")),
        };
        if self.predictor != 1 {
            return Err(format!("unsupported COG predictor {} (only predictor=1/none is supported)", self.predictor));
        }
        Ok(comp)
    }

    /// Parse a GDAL_NODATA ASCII tag value to a number (returns None for NaN/unparseable).
    pub fn parse_nodata(s: &str) -> Option<f64> {
        let t = s.trim().trim_end_matches('\0').trim();
        match t.parse::<f64>() {
            Ok(v) if v.is_finite() => Some(v),
            _ => None,
        }
    }
}
```
In `parse_cog_metadata`, add an ASCII reader for tag 42113 (type 2 = ASCII; bytes at `val_or_offset`, length `count`), and a match arm:
```rust
42113 => {
    let start = val_or_offset as usize;
    let end = (start + count as usize).min(buffer.len());
    if start <= end {
        if let Ok(s) = std::str::from_utf8(&buffer[start..end]) {
            meta.nodata = CogMetadata::parse_nodata(s);
        }
    }
}
```
(Add `pub nodata: Option<f64>` to the struct if not already; `Default` gives `None`.)

- [ ] **Step 4: Run to pass**; Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add geozarr_core/src/cog.rs
git commit --no-gpg-sign -m "feat(cog): compression/predictor support detection and nodata parsing

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Georeferencing — affine + CRS + dimension names

**Files:** Modify `geozarr_core/src/cog.rs`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_georeferencing() {
    use crate::metadata::SpatialTransform;
    // ModelPixelScale=[2.0,2.0,0], ModelTiepoint=[0,0,0, -180,90,0], EPSG via GeoKey 2048=4326
    let mut b = vec![0u8; 300];
    b[0..2].copy_from_slice(b"II");
    b[2..4].copy_from_slice(&42u16.to_le_bytes());
    b[4..8].copy_from_slice(&8u32.to_le_bytes());
    b[8..10].copy_from_slice(&3u16.to_le_bytes()); // 3 entries
    let mut o = 10;
    // entry helper for arrays pointing at an offset
    let mut put = |b: &mut [u8], o: usize, tag: u16, typ: u16, count: u32, voff: u32| {
        b[o..o+2].copy_from_slice(&tag.to_le_bytes());
        b[o+2..o+4].copy_from_slice(&typ.to_le_bytes());
        b[o+4..o+8].copy_from_slice(&count.to_le_bytes());
        b[o+8..o+12].copy_from_slice(&voff.to_le_bytes());
    };
    // place data after the IFD (IFD ends at 10 + 3*12 = 46)
    let scale_off = 48usize; // 3 doubles = 24 bytes
    for (i, v) in [2.0f64, 2.0, 0.0].iter().enumerate() {
        b[scale_off + i*8..scale_off + i*8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    let tp_off = 72usize; // 6 doubles = 48 bytes
    for (i, v) in [0.0f64,0.0,0.0,-180.0,90.0,0.0].iter().enumerate() {
        b[tp_off + i*8..tp_off + i*8 + 8].copy_from_slice(&v.to_le_bytes());
    }
    let gk_off = 120usize; // GeoKeyDirectory SHORTs: header(4) + 1 key(4) = 8 shorts
    let gk: [u16;8] = [1,1,0,1, 2048,0,1,4326];
    for (i, v) in gk.iter().enumerate() {
        b[gk_off + i*2..gk_off + i*2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    put(&mut b, o, 33550, 12, 3, scale_off as u32); o += 12; // ModelPixelScale (DOUBLE)
    put(&mut b, o, 33922, 12, 6, tp_off as u32); o += 12;     // ModelTiepoint (DOUBLE)
    put(&mut b, o, 34735, 3, 8, gk_off as u32);               // GeoKeyDirectory (SHORT)
    let m = parse_cog_metadata(&b).unwrap();
    let t: SpatialTransform = m.spatial_transform().unwrap();
    assert_eq!(t.scale, vec![-2.0, 2.0]);          // [lat(row), lon(col)]
    assert_eq!(t.translation, vec![90.0, -180.0]); // [tiepointY, tiepointX]
    assert_eq!(m.crs(), Some("EPSG:4326".to_string()));
    assert_eq!(m.dim_names(), vec!["lat".to_string(), "lon".to_string()]);
}
```

- [ ] **Step 2: Run (fail)**; Expected: FAIL (no `spatial_transform`/`crs`/`dim_names`; DOUBLE/GeoKey not parsed).

- [ ] **Step 3: Implement**

Add fields to `CogMetadata`: `pub pixel_scale: Vec<f64>`, `pub tiepoint: Vec<f64>`, `pub model_transformation: Vec<f64>`, `pub epsg: Option<u32>`. In `parse_cog_metadata`, add a DOUBLE-array extractor and match arms:
```rust
let extract_f64_array = |count: usize, offset_val: u32| -> Vec<f64> {
    let mut res = Vec::with_capacity(count);
    let mut ptr = offset_val as usize;
    for _ in 0..count {
        if ptr + 8 > buffer.len() { break; }
        let v = if header.is_little_endian {
            f64::from_le_bytes(buffer[ptr..ptr + 8].try_into().unwrap())
        } else {
            f64::from_be_bytes(buffer[ptr..ptr + 8].try_into().unwrap())
        };
        ptr += 8; res.push(v);
    }
    res
};
let extract_u16_array = |count: usize, offset_val: u32| -> Vec<u16> {
    let mut res = Vec::with_capacity(count);
    let mut ptr = offset_val as usize;
    for _ in 0..count {
        if ptr + 2 > buffer.len() { break; }
        let v = if header.is_little_endian {
            u16::from_le_bytes(buffer[ptr..ptr + 2].try_into().unwrap())
        } else {
            u16::from_be_bytes(buffer[ptr..ptr + 2].try_into().unwrap())
        };
        ptr += 2; res.push(v);
    }
    res
};
```
Match arms:
```rust
33550 => meta.pixel_scale = extract_f64_array(count as usize, val_or_offset),
33922 => meta.tiepoint = extract_f64_array(count as usize, val_or_offset),
34264 => meta.model_transformation = extract_f64_array(count as usize, val_or_offset),
34735 => {
    let keys = extract_u16_array(count as usize, val_or_offset);
    // header is keys[0..4]; entries are 4-tuples [KeyID, Location, Count, Value]
    let mut i = 4;
    while i + 4 <= keys.len() {
        let key_id = keys[i];
        let location = keys[i + 1];
        let value = keys[i + 3];
        if (key_id == 3072 || key_id == 2048) && location == 0 {
            meta.epsg = Some(value as u32);
        }
        i += 4;
    }
}
```
Add methods:
```rust
impl CogMetadata {
    /// North-up affine as a SpatialTransform with dims [row(lat), col(lon)].
    pub fn spatial_transform(&self) -> Option<crate::metadata::SpatialTransform> {
        if self.pixel_scale.len() >= 2 && self.tiepoint.len() >= 6 {
            let sx = self.pixel_scale[0];
            let sy = self.pixel_scale[1];
            let tx = self.tiepoint[3];
            let ty = self.tiepoint[4];
            return Some(crate::metadata::SpatialTransform {
                scale: vec![-sy, sx],
                translation: vec![ty, tx],
            });
        }
        if self.model_transformation.len() >= 16 {
            // 4x4 row-major: x' = a*col + b*row + ... ; use diagonal + translation column
            let m = &self.model_transformation;
            let sx = m[0]; // col scale (x)
            let sy = m[5]; // row scale (y), already signed
            let tx = m[3];
            let ty = m[7];
            return Some(crate::metadata::SpatialTransform {
                scale: vec![sy, sx],
                translation: vec![ty, tx],
            });
        }
        None
    }
    pub fn crs(&self) -> Option<String> { self.epsg.map(|c| format!("EPSG:{c}")) }
    /// Geographic CRS → ["lat","lon"] (enables lat_min/lon_max pushdown);
    /// any other/absent CRS → ["y","x"].
    pub fn dim_names(&self) -> Vec<String> {
        match self.epsg {
            Some(4326) => vec!["lat".into(), "lon".into()],
            _ => vec!["y".into(), "x".into()],
        }
    }
}
```

- [ ] **Step 4: Run to pass**; Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add geozarr_core/src/cog.rs
git commit --no-gpg-sign -m "feat(cog): parse GeoTIFF affine and CRS; derive spatial transform + dims

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Synthesize honest metadata in `VirtualCogStore`

**Files:** Modify `geozarr_core/src/virtual_store.rs`.

- [ ] **Step 1: Write/replace the failing test**

Replace `test_virtual_store_metadata` with one asserting the real dtype, dims, transform, and CRS appear:
```rust
#[tokio::test]
async fn test_virtual_store_synthesizes_geozarr_attrs() {
    use zarrs::storage::ReadableStorageTraits;
    let mut meta = crate::cog::CogMetadata {
        image_width: 4, image_length: 2, tile_width: 4, tile_length: 2,
        tile_offsets: vec![0], tile_byte_counts: vec![16],
        is_little_endian: true, bits_per_sample: 16, sample_format: 2,
        samples_per_pixel: 1, compression: 1, predictor: 1,
        ..Default::default()
    };
    meta.pixel_scale = vec![2.0, 2.0, 0.0];
    meta.tiepoint = vec![0.0, 0.0, 0.0, -180.0, 90.0, 0.0];
    meta.epsg = Some(4326);

    let op = opendal::Operator::new(opendal::services::Memory::default()).unwrap().finish();
    let store = VirtualCogStore::new(op, "".to_string(), meta);

    let zarray = String::from_utf8(
        store.get(&zarrs::storage::StoreKey::new(".zarray").unwrap()).unwrap().unwrap().to_vec()
    ).unwrap();
    assert!(zarray.contains("\"<i2\""), "dtype should be <i2: {zarray}");

    let zattrs = String::from_utf8(
        store.get(&zarrs::storage::StoreKey::new(".zattrs").unwrap()).unwrap().unwrap().to_vec()
    ).unwrap();
    assert!(zattrs.contains("_ARRAY_DIMENSIONS"));
    assert!(zattrs.contains("\"lat\"") && zattrs.contains("\"lon\""));
    assert!(zattrs.contains("EPSG:4326"));
    assert!(zattrs.contains("spatial_transform"));
}
```

- [ ] **Step 2: Run (fail)**; Expected: FAIL (dtype hardcoded `<f4`; no `.zattrs`).

- [ ] **Step 3: Implement synthesis**

In `VirtualCogStore::new`, precompute the dtype string, fill value JSON, dim-name JSON array, and a `.zattrs` JSON; store them as `Bytes` fields. Add fields to the struct: `zarray_bytes: Bytes`, `zattrs_bytes: Bytes` (keep `zmetadata_bytes`). Build them:
```rust
let dtype = meta.zarr_dtype().unwrap_or_else(|_| "<f4".to_string());
let fill = match meta.nodata {
    Some(v) => format!("{v}"),
    None => "null".to_string(),
};
let dims = meta.dim_names(); // ["lat","lon"] or ["y","x"]
let dims_json = format!("[\"{}\", \"{}\"]", dims[0], dims[1]);
let geozarr = match (meta.spatial_transform(), meta.crs()) {
    (Some(t), crs) => {
        let crs_json = crs.map(|c| format!("\"crs\": \"{c}\",")).unwrap_or_default();
        format!(
            "{{ {} \"spatial_transform\": {{ \"scale\": [{}, {}], \"translation\": [{}, {}] }} }}",
            crs_json, t.scale[0], t.scale[1], t.translation[0], t.translation[1]
        )
    }
    (None, _) => "{}".to_string(),
};
let zarray = format!(
    r#"{{"zarr_format":2,"shape":[{},{}],"chunks":[{},{}],"dtype":"{}","compressor":null,"fill_value":{},"filters":null,"order":"C"}}"#,
    meta.image_length, meta.image_width, meta.tile_length, meta.tile_width, dtype, fill
);
let zattrs = format!(r#"{{"_ARRAY_DIMENSIONS":{},"geozarr":{}}}"#, dims_json, geozarr);
let zmetadata = format!(
    r#"{{"metadata":{{".zarray":{},".zattrs":{}}},"zarr_consolidated_format":1}}"#,
    zarray, zattrs
);
```
Store `zarray_bytes`, `zattrs_bytes`, `zmetadata_bytes` and return them from `get()` for `.zarray`/`.zattrs`/`.zmetadata`. Update `list()` to include all three keys:
```rust
fn list(&self) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
    Ok(vec![
        StoreKey::new(".zmetadata").unwrap(),
        StoreKey::new(".zarray").unwrap(),
        StoreKey::new(".zattrs").unwrap(),
    ])
}
```
Remove the old inline hardcoded `<f4`/`"NaN"` JSON in `get()` — serve the precomputed `Bytes` instead.

- [ ] **Step 4: Run to pass**; Expected: PASS.
- [ ] **Step 5: Commit**

```bash
git add geozarr_core/src/virtual_store.rs
git commit --no-gpg-sign -m "feat(cog): synthesize real dtype + geozarr attrs (dims, transform, CRS)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Decode Deflate tiles in the store layer

**Files:** Modify `geozarr_core/src/virtual_store.rs`, `geozarr_core/Cargo.toml`.

- [ ] **Step 1: Add `flate2`**

In `geozarr_core/Cargo.toml` under `[dependencies]`:
```toml
flate2 = "1"
```
Run: `cargo build -p geozarr_core` → Expected: builds.

- [ ] **Step 2: Write the failing test**

```rust
#[tokio::test]
async fn test_deflate_tile_is_inflated() {
    use zarrs::storage::ReadableStorageTraits;
    use std::io::Write;
    // raw 2x4 i16 LE tile = 16 bytes
    let raw: Vec<u8> = (0..16u8).collect();
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).unwrap();
    let compressed = enc.finish().unwrap();

    let op = opendal::Operator::new(opendal::services::Memory::default()).unwrap().finish();
    op.write("tile.bin", compressed.clone()).await.unwrap();

    let meta = crate::cog::CogMetadata {
        image_width: 4, image_length: 2, tile_width: 4, tile_length: 2,
        tile_offsets: vec![0], tile_byte_counts: vec![compressed.len() as u64],
        is_little_endian: true, bits_per_sample: 16, sample_format: 2,
        samples_per_pixel: 1, compression: 8, predictor: 1, ..Default::default()
    };
    let store = VirtualCogStore::new(op, "tile.bin".to_string(), meta);
    let out = store.get(&zarrs::storage::StoreKey::new("0.0").unwrap()).unwrap().unwrap();
    assert_eq!(out.to_vec(), raw, "deflate tile must be inflated to raw bytes");
}
```

- [ ] **Step 3: Run (fail)**; Expected: FAIL (raw compressed bytes returned).

- [ ] **Step 4: Implement decode in `get()`**

After fetching `bytes` for a chunk and before returning, branch on the compression kind:
```rust
if let Ok(raw) = bytes_res {
    let raw = raw.to_vec();
    let decoded = match self.meta.compression_kind() {
        Ok(crate::cog::CogCompression::None) => raw,
        Ok(crate::cog::CogCompression::Deflate) => {
            use std::io::Read;
            let mut d = flate2::read::ZlibDecoder::new(&raw[..]);
            let mut out = Vec::new();
            d.read_to_end(&mut out).map_err(|e| {
                zarrs::storage::StorageError::Other(format!("deflate decode failed: {e}"))
            })?;
            out
        }
        Err(e) => return Err(zarrs::storage::StorageError::Other(e)),
    };
    return Ok(Some(Bytes::from(decoded)));
}
```
(If `StorageError::Other` is not a variant, use the crate's available constructor — check `zarrs::storage::StorageError` and use the string/`from` path the rest of the file uses. The existing code converts errors via `.to_string()`; match that idiom.)

- [ ] **Step 5: Run to pass**; Expected: PASS.
- [ ] **Step 6: Commit**

```bash
git add geozarr_core/src/virtual_store.rs geozarr_core/Cargo.toml geozarr_core/Cargo.lock
git commit --no-gpg-sign -m "feat(cog): inflate Deflate-compressed tiles before decode

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Committed GeoTIFF fixtures + generator

**Files:** Create `scripts/generate_cog_fixture.py`; Create `geozarr_core/tests/fixtures/*.tif`.

- [ ] **Step 1: Write the generator**

Create `scripts/generate_cog_fixture.py`:
```python
"""Generate tiny deterministic COG fixtures for geozarr_core tests.

Run once to (re)produce the committed fixtures:
    pip install rasterio numpy
    python scripts/generate_cog_fixture.py

Emits, under geozarr_core/tests/fixtures/:
  - cog_int16_uncompressed.tif  (EPSG:4326, 4x2, Int16, no compression, predictor=1)
  - cog_int16_deflate.tif       (same data, Deflate, predictor=1)
Affine: origin (-180, 90), pixel size 2.0; so lon = -180 + 2*col, lat = 90 - 2*row.
Values: v[row, col] = row*10 + col  -> deterministic, easy to assert.
"""
import os
import numpy as np
import rasterio
from rasterio.transform import from_origin

OUT = os.path.join(os.path.dirname(__file__), "..", "geozarr_core", "tests", "fixtures")
os.makedirs(OUT, exist_ok=True)

data = np.array([[0, 1, 2, 3], [10, 11, 12, 13]], dtype=np.int16)  # rows=2, cols=4
transform = from_origin(-180.0, 90.0, 2.0, 2.0)  # west, north, xsize, ysize

def write(path, **extra):
    with rasterio.open(
        path, "w", driver="GTiff", height=2, width=4, count=1,
        dtype="int16", crs="EPSG:4326", transform=transform,
        tiled=True, blockxsize=16, blockysize=16, predictor=1, **extra,
    ) as dst:
        dst.write(data, 1)

write(os.path.join(OUT, "cog_int16_uncompressed.tif"), compress="none")
write(os.path.join(OUT, "cog_int16_deflate.tif"), compress="deflate")
print("Wrote fixtures to", os.path.abspath(OUT))
```
> Note: rasterio may emit a single tile for a 4x2 image even with blocksize 16 (tile padded to image) — that's fine; the store handles a single tile (`0.0`). If rasterio rounds the internal tiling differently, the e2e test in Task 8 reads whatever tiling results; do not hardcode tile counts.

- [ ] **Step 2: Generate and commit the fixtures**

Run:
```bash
python3 -m pip install --quiet rasterio numpy 2>/dev/null || pip install rasterio numpy
python3 scripts/generate_cog_fixture.py
ls -la geozarr_core/tests/fixtures/
```
Expected: two `.tif` files (each a few KB).
**If rasterio cannot be installed in this environment:** stop and report `BLOCKED` with that reason — the controller will generate and commit the fixtures. Do not fabricate a `.tif`.

- [ ] **Step 3: Commit**

```bash
git add scripts/generate_cog_fixture.py geozarr_core/tests/fixtures/cog_int16_uncompressed.tif geozarr_core/tests/fixtures/cog_int16_deflate.tif
git commit --no-gpg-sign -m "test(cog): add generator and committed GeoTIFF fixtures

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: End-to-end georeferenced read tests

**Files:** Create `geozarr_core/tests/cog_e2e.rs`.

- [ ] **Step 1: Write the failing tests**

```rust
use geozarr_core::dataset::ZarrDataset;
use geozarr_core::query_planner::QueryConstraints;
use std::collections::HashMap;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

#[test]
fn cog_metadata_is_georeferenced() {
    let ds = ZarrDataset::open(&fixture("cog_int16_uncompressed.tif")).unwrap();
    assert_eq!(ds.dim_names, vec!["lat".to_string(), "lon".to_string()]);
    assert!(ds.spatial_transform.is_some(), "affine transform must be present");
    let schema = ds.schema().unwrap();
    // value column dtype is Int16 (not Float32)
    let (vname, vtype) = schema.last().unwrap();
    assert_eq!(vname, "value");
    assert_eq!(format!("{vtype:?}"), format!("{:?}", zarrs::array::DataType::Int16));
}

#[test]
fn cog_bbox_prunes_via_lat_lon() {
    let ds = ZarrDataset::open(&fixture("cog_int16_uncompressed.tif")).unwrap();
    // Full extent: lon in [-180,-174], lat in [86,90] (origin -180/90, 2deg, 4x2).
    // Constrain to the western half (lon <= -177) -> fewer columns.
    let mut bounds = HashMap::new();
    bounds.insert("lon".to_string(), (None, Some(-177.0)));
    let constraints = QueryConstraints { bounds, pins: HashMap::new() };
    let (bmin, bmax) = ds.compute_bounds(&constraints);
    // lon dim is index 1; with scale +2 translation -180, lon=-177 -> col ~1.5 -> max col 1
    assert!(bmax[1] < (ds.shape[1] - 1), "bbox should prune the lon dimension: {bmin:?}..{bmax:?}");
}

#[test]
fn cog_deflate_matches_uncompressed_metadata() {
    let a = ZarrDataset::open(&fixture("cog_int16_uncompressed.tif")).unwrap();
    let b = ZarrDataset::open(&fixture("cog_int16_deflate.tif")).unwrap();
    assert_eq!(a.shape, b.shape);
    assert_eq!(a.dim_names, b.dim_names);
}
```
> If `ZarrDataset` fields (`dim_names`, `shape`, `spatial_transform`) aren't `pub`, use the public accessors the crate exposes (check `dataset.rs`); the e2e intent is: dims are `lat`/`lon`, dtype is Int16, and a `lon`-bounded `compute_bounds` prunes. Adjust field/method access to what's public without weakening the assertions.

- [ ] **Step 2: Run (fail if any wiring is off)**

Run: `cargo test -p geozarr_core --test cog_e2e`
Expected: initially may FAIL — iterate on the synthesis (Task 5) until georeferencing flows. Likely fix points: `.zattrs` must be discoverable by `Array::open` (ensure `.zmetadata` consolidated includes `.zattrs`, and `get(".zattrs")` works); dtype string must be one `zarrs` accepts.

- [ ] **Step 3: Make them pass**

Iterate on Tasks 5/6 code as needed (no new files). Confirm `read_zarr_metadata`-level data is right by also opening via `ZarrDataset` here.

- [ ] **Step 4: Full crate test + clippy + fmt**

```bash
cargo test -p geozarr_core
cargo clippy -p geozarr_core --all-targets -- -D warnings
cargo fmt --check
```
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add geozarr_core/tests/cog_e2e.rs
git commit --no-gpg-sign -m "test(cog): end-to-end georeferenced read, dtype, and bbox pruning

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Docs — flip COG to first-class

**Files:** Modify `docs/docs/engineering/cog_virtualization.mdx`, `docs/docs/usage/sql_read_geo.md`.

- [ ] **Step 1: Update the engineering page**

In `docs/docs/engineering/cog_virtualization.mdx`, replace the `:::caution Experimental` admonition with a first-class statement, and add an honest "Supported / Not yet supported" subsection:
```markdown
:::note Status
COG is a first-class `read_geo` source: `read_geo('path.tif')` and
`read_zarr_metadata('path.tif')` return georeferenced, type-correct results.
:::
```
Add after the mechanism section:
```markdown
## Supported & limitations

**Supported:** single-band GeoTIFFs; uncompressed and Deflate-compressed tiles
(predictor=1); the GeoTIFF affine (`ModelPixelScale`/`ModelTiepoint` or
`ModelTransformation`) and CRS (`GeoKeyDirectory`). For a geographic CRS
(EPSG:4326) the dimensions are `lat`/`lon`, so `lat_min`/`lon_max` bounding-box
pushdown applies; the value column uses the COG's real data type.

**Not yet supported:** multi-band COGs; LZW/JPEG/WebP internal compression;
horizontal-differencing predictors; CRS reprojection (projected COGs are read
in their native CRS with `y`/`x` dimensions, so geographic bbox pushdown does
not apply). These return a clear error or fall back to unfiltered reads.
```
Keep the STAC section as "planned / not wired to SQL."

- [ ] **Step 2: Update the SQL reference source list**

In `docs/docs/usage/sql_read_geo.md`, find the "source kinds / supported URIs" content and mark COG (`.tif`/`.tiff`) as **supported** (single-band, uncompressed/Deflate, georeferenced; STAC still experimental/absent). Keep wording consistent with the engineering page's limits.

- [ ] **Step 3: Build the docs**

Run: `cd docs && (test -d node_modules || npm ci) && npm run build 2>&1 | tail -5`
Expected: `[SUCCESS]`, no broken links.

- [ ] **Step 4: Commit**

```bash
git add docs/docs/engineering/cog_virtualization.mdx docs/docs/usage/sql_read_geo.md
git commit --no-gpg-sign -m "docs: mark COG as a first-class read_geo source with explicit limits

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Full verification

**Files:** none (verification only).

- [ ] **Step 1: Whole workspace green**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
Expected: all pass (existing tests unaffected; new COG tests pass). The existing live-network STAC test is unchanged.

- [ ] **Step 2: Docs build green** — `cd docs && npm run build 2>&1 | tail -5` → `[SUCCESS]`.

- [ ] **Step 3: Scope & no-dispatch-change check**

Run: `git diff --name-status origin/main..HEAD` — expected set:
```
M geozarr_core/src/cog.rs
M geozarr_core/src/virtual_store.rs
M geozarr_core/Cargo.toml
M geozarr_core/Cargo.lock
A scripts/generate_cog_fixture.py
A geozarr_core/tests/fixtures/cog_int16_uncompressed.tif
A geozarr_core/tests/fixtures/cog_int16_deflate.tif
A geozarr_core/tests/cog_e2e.rs
M docs/docs/engineering/cog_virtualization.mdx
M docs/docs/usage/sql_read_geo.md
A docs/superpowers/plans/2026-06-07-cog-first-class.md
A docs/superpowers/specs/2026-06-07-cog-first-class-design.md
```
Confirm `extension/src/table_function.rs` is **NOT** in the diff (no dispatch/short-circuit change), and the STAC short-circuit is untouched.

---

## Self-review notes (author checklist, already applied)

- **Spec coverage:** dtype/fill/compression/bands/affine/CRS parsing (Tasks 1–4) ✓; honest synthesis + Deflate decode (Tasks 5–6) ✓; committed generated fixture (Task 7) ✓; offline unit + e2e tests incl. bbox pruning and Deflate==uncompressed (Tasks 1–8) ✓; docs flip with explicit limits (Task 9) ✓; CI + scope + no-dispatch-change gate (Task 10) ✓; STAC and reprojection explicitly out ✓.
- **Naming/integration consistency:** `CogMetadata` fields and methods (`zarr_dtype`, `compression_kind`, `parse_nodata`, `spatial_transform`, `crs`, `dim_names`) are defined in Tasks 1–4 and consumed in Task 5; `SpatialTransform { scale, translation }` matches `metadata.rs`; dim names `lat`/`lon` align with `read_geo`'s `{name}_min` param binding and the affine branch of `compute_bounds`.
- **Placeholders:** none. The two soft spots are explicit, justified verification points: (a) the exact `StorageError` constructor (Task 6 — match the file's existing error idiom), and (b) `ZarrDataset` field visibility in the e2e test (Task 8 — use whatever is public without weakening assertions). The fixture has a documented BLOCKED fallback if rasterio is unavailable.
- **TDD:** every code task is failing-test → run → implement → pass → commit; frequent small commits.
- **Non-goals honored:** no dispatch change, no STAC work, no multi-band/LZW/JPEG/predictor/reprojection, no unrelated refactor (the per-chunk tokio runtime wart is left as documented future work).
