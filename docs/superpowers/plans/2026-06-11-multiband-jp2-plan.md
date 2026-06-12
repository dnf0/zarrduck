# Multi-band COGs and JP2 Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add native support for Multi-band COGs and graceful fallback error messages pointing users to DuckDB `st_read` for unsupported formats like JP2.

**Architecture:** We will update `cog.rs` to allow parsing `SamplesPerPixel > 1`. We will update `virtual_store.rs` to inject a `band` dimension into Zarr metadata and correctly map `0.y.x` chunk requests. The pixel bytes will be de-interleaved into a planar layout on-the-fly during chunk fetching. Finally, we will update the `cog.rs` parser to return a helpful error suggesting `st_read()` for JP2 files.

**Tech Stack:** Rust, DuckDB, Zarr

---

### Task 1: Enable Multi-band Support in COG Parser

**Files:**
- Modify: `geozarr_core/src/cog.rs`

- [x] **Step 1: Remove `samples_per_pixel` check in `zarr_dtype`**
In `geozarr_core/src/cog.rs`, find the `zarr_dtype` function (around line 70) and remove the early return error for `samples_per_pixel != 1`.

```rust
    pub fn zarr_dtype(&self) -> Result<String, String> {
        let endian = if self.bits_per_sample <= 8 {
            "|"
        } else if self.is_little_endian {
            "<"
        } else {
            ">"
        };
        let kind = match self.sample_format {
            3 => "f", // float
            2 => "i", // signed int
            1 => "u", // unsigned int
            other => return Err(format!("unsupported TIFF SampleFormat {other}")),
        };
        let bytes = match self.bits_per_sample {
            8 => "1",
            16 => "2",
            32 => "4",
            64 => "8",
            other => return Err(format!("unsupported TIFF BitsPerSample {other}")),
        };
        Ok(format!("{}{}{}", endian, kind, bytes))
    }
```

- [x] **Step 2: Update Fallback Errors in `cog.rs`**
Update the TIFF header validation in `parse_tiff_header` (around line 11):
```rust
    let is_little_endian = match &buffer[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("Invalid TIFF byte order (likely a JP2 or other unsupported format). Please use DuckDB's native st_read() via the spatial extension instead.".into()),
    };
```

And in `compression_kind` (around line 111):
```rust
            other => {
                return Err(format!(
                "unsupported COG compression {other} (only uncompressed and Deflate are supported). Please use DuckDB's native st_read() via the spatial extension instead."
            ))
            }
```

- [x] **Step 3: Run `cargo check`**
Run: `cargo check -p geozarr_core`
Expected: Passes. Note: test `test_zarr_dtype_and_band_guard` may fail since we removed the guard.

- [x] **Step 4: Fix `test_zarr_dtype_and_band_guard`**
In `geozarr_core/src/cog.rs` (around line 464), remove the lines asserting `zarr_dtype()` is err for multiband. Delete the `bad` variable and the `assert!(bad.zarr_dtype().is_err());` line. Make sure `cargo test -p geozarr_core` passes.

- [x] **Step 5: Commit**
```bash
git add geozarr_core/src/cog.rs
git commit -m "feat(cog): allow multiband parsing and add jp2 fallback hints"
```

### Task 2: Inject Band Dimension in Virtual Store

**Files:**
- Modify: `geozarr_core/src/virtual_store.rs`

- [x] **Step 1: Update metadata generation**
In `geozarr_core/src/virtual_store.rs`, in the `VirtualCogStore::new()` method (around line 35), replace the `dims_json`, `zarray`, and `zattrs` formatting blocks to conditionally include bands.

```rust
        let dims = meta.dim_names(); // ["lat","lon"] or ["y","x"]
        let dims_json = if meta.samples_per_pixel > 1 {
            format!("[\"band\", \"{}\", \"{}\"]", dims[0], dims[1])
        } else {
            format!("[\"{}\", \"{}\"]", dims[0], dims[1])
        };

        let (shape_json, chunks_json) = if meta.samples_per_pixel > 1 {
            (
                format!("[{},{},{}]", meta.samples_per_pixel, meta.image_length, meta.image_width),
                format!("[{},{},{}]", meta.samples_per_pixel, meta.tile_length, meta.tile_width),
            )
        } else {
            (
                format!("[{},{}]", meta.image_length, meta.image_width),
                format!("[{},{}]", meta.tile_length, meta.tile_width),
            )
        };

        let geozarr = match (meta.spatial_transform(), meta.crs()) {
            (Some(t), crs) => {
                let crs_json = crs
                    .map(|c| format!("\"crs\": \"{c}\","))
                    .unwrap_or_default();
                format!(
                    "{{ {} \"spatial_transform\": {{ \"scale\": [{}, {}], \"translation\": [{}, {}] }} }}",
                    crs_json, t.scale[0], t.scale[1], t.translation[0], t.translation[1]
                )
            }
            (None, _) => "{}".to_string(),
        };

        let zarray = format!(
            r#"{{"zarr_format":2,"shape":{},"chunks":{},"dtype":"{}","compressor":null,"fill_value":{},"filters":null,"order":"C"}}"#,
            shape_json, chunks_json, dtype, fill
        );
        let zattrs = format!(
            r#"{{"_ARRAY_DIMENSIONS":{},"geozarr":{}}}"#,
            dims_json, geozarr
        );
```

- [x] **Step 2: Commit**
```bash
git add geozarr_core/src/virtual_store.rs
git commit -m "feat(store): expose multi-band shape in zarr metadata"
```

### Task 3: De-interleave Pixel Data in Virtual Store

**Files:**
- Modify: `geozarr_core/src/virtual_store.rs`

- [x] **Step 1: Update Chunk Key parsing**
In `geozarr_core/src/virtual_store.rs` `VirtualCogStore::get()`, we need to ignore the `band` index in the chunk key because a single COG tile provides all bands.

Replace `let chunks: Vec<&str> = key.as_str().split('.').collect();` with:
```rust
        let mut chunks: Vec<&str> = key.as_str().split('.').collect();
        if self.meta.samples_per_pixel > 1 && chunks.len() == 3 && chunks[0] == "0" {
            chunks.remove(0); // pop the band dimension to get ['y', 'x']
        }
```

- [x] **Step 2: Add planar de-interleaving**
Inside `VirtualCogStore::get()`, right after `let mut decoded = match self.meta.compression_kind() { ... };` (around line 128), add the de-interleaving logic before `return Ok(Some(Bytes::from(decoded)));`:

```rust
                        if self.meta.samples_per_pixel > 1 {
                            let spp = self.meta.samples_per_pixel as usize;
                            let bytes_per_sample = (self.meta.bits_per_sample / 8) as usize;
                            let pixel_stride = spp * bytes_per_sample;
                            let num_pixels = decoded.len() / pixel_stride;

                            let mut planar = vec![0u8; decoded.len()];
                            for band in 0..spp {
                                for p in 0..num_pixels {
                                    let src_idx = p * pixel_stride + band * bytes_per_sample;
                                    let dst_idx = band * (num_pixels * bytes_per_sample) + p * bytes_per_sample;
                                    if src_idx + bytes_per_sample <= decoded.len() && dst_idx + bytes_per_sample <= planar.len() {
                                        planar[dst_idx..dst_idx + bytes_per_sample]
                                            .copy_from_slice(&decoded[src_idx..src_idx + bytes_per_sample]);
                                    }
                                }
                            }
                            decoded = planar;
                        }
```

- [x] **Step 3: Test and Commit**
Run `cargo test -p geozarr_core`
If everything passes:
```bash
git add geozarr_core/src/virtual_store.rs
git commit -m "feat(store): de-interleave pixels to planar layout for multiband"
```
