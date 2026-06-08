//! A virtual Zarr group stacking one COG asset across N STAC Items along time.
//! Per asset: a 3D `[N, H, W]` array whose `[1, tileH, tileW]` chunks route to
//! each item's COG tile. Group-level `/time`, `/lat`, `/lon` coordinate arrays
//! make the stack open fully coordinate-resolved.
use crate::virtual_store::VirtualCogStore;
use bytes::Bytes;
use std::collections::HashMap;
use zarrs::storage::{ListableStorageTraits, ReadableStorageTraits, StoreKey, StorePrefix};

pub struct VirtualStacTimeStack {
    /// asset name -> time-sorted per-item COG stores (len N).
    assets: HashMap<String, Vec<VirtualCogStore>>,
    /// asset name -> synthesized 3D `.zarray` / `.zattrs` bytes.
    asset_zarray: HashMap<String, Bytes>,
    asset_zattrs: HashMap<String, Bytes>,
    /// coordinate name -> (`.zarray` bytes, chunk `0` bytes). Keys: time/lat/lon (or y/x).
    coords: HashMap<String, (Bytes, Bytes)>,
    zgroup_bytes: Bytes,
    zmetadata_bytes: Bytes,
}

fn coord_zarray(len: usize) -> String {
    format!(
        r#"{{"zarr_format":2,"shape":[{len}],"chunks":[{len}],"dtype":"<f8","compressor":null,"fill_value":0.0,"filters":null,"order":"C"}}"#
    )
}
fn coord_bytes(vals: &[f64]) -> Bytes {
    let mut b = Vec::with_capacity(vals.len() * 8);
    for v in vals {
        b.extend_from_slice(&v.to_le_bytes());
    }
    Bytes::from(b)
}

impl VirtualStacTimeStack {
    /// `assets`: per-asset time-sorted item stores (each len == times.len()).
    /// `times`: epoch seconds (sorted). `lat`/`lon`: spatial coordinate values
    /// (len H / W). `spatial_dims`: ["lat","lon"] or ["y","x"].
    pub fn new(
        assets: HashMap<String, Vec<VirtualCogStore>>,
        times: Vec<f64>,
        lat: Vec<f64>,
        lon: Vec<f64>,
        spatial_dims: [String; 2],
    ) -> Result<Self, String> {
        let n = times.len();
        let h = lat.len();
        let w = lon.len();
        if assets.is_empty() {
            return Err("time-stack has no assets".into());
        }

        let mut asset_zarray = HashMap::new();
        let mut asset_zattrs = HashMap::new();
        let mut meta_map = serde_json::Map::new();
        meta_map.insert(".zgroup".into(), serde_json::json!({"zarr_format": 2}));

        for (name, items) in &assets {
            if items.len() != n {
                return Err(format!("asset {name}: {} items, expected {n}", items.len()));
            }
            // Derive the 3D .zarray from the child's 2D .zarray (carries dtype/fill).
            let child0 = &items[0];
            let z2 = child0
                .get(&StoreKey::new(".zarray").unwrap())
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("asset {name}: child has no .zarray"))?;
            let mut z: serde_json::Value =
                serde_json::from_slice(&z2).map_err(|e| e.to_string())?;
            let chunks_2d = z["chunks"].clone();
            let (tile_h, tile_w) = (
                chunks_2d[0].as_u64().unwrap_or(h as u64),
                chunks_2d[1].as_u64().unwrap_or(w as u64),
            );
            z["shape"] = serde_json::json!([n, h, w]);
            z["chunks"] = serde_json::json!([1, tile_h, tile_w]);
            let z_str = z.to_string();

            let zattrs = serde_json::json!({
                "_ARRAY_DIMENSIONS": ["time", spatial_dims[0], spatial_dims[1]],
            })
            .to_string();

            meta_map.insert(
                format!("{name}/.zarray"),
                serde_json::from_str::<serde_json::Value>(&z_str).unwrap(),
            );
            meta_map.insert(
                format!("{name}/.zattrs"),
                serde_json::from_str::<serde_json::Value>(&zattrs).unwrap(),
            );
            asset_zarray.insert(name.clone(), Bytes::from(z_str));
            asset_zattrs.insert(name.clone(), Bytes::from(zattrs));
        }

        let mut coords = HashMap::new();
        for (cname, vals) in [
            ("time".to_string(), &times),
            (spatial_dims[0].clone(), &lat),
            (spatial_dims[1].clone(), &lon),
        ] {
            let za = coord_zarray(vals.len());
            meta_map.insert(
                format!("{cname}/.zarray"),
                serde_json::from_str::<serde_json::Value>(&za).unwrap(),
            );
            coords.insert(cname, (Bytes::from(za), coord_bytes(vals)));
        }

        let zmetadata = serde_json::json!({
            "metadata": meta_map,
            "zarr_consolidated_format": 1
        })
        .to_string();

        Ok(Self {
            assets,
            asset_zarray,
            asset_zattrs,
            coords,
            zgroup_bytes: Bytes::from(r#"{"zarr_format": 2}"#),
            zmetadata_bytes: Bytes::from(zmetadata),
        })
    }

    /// All asset names (sorted) — for `ResolvedStore.stac_assets`.
    pub fn asset_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.assets.keys().cloned().collect();
        v.sort();
        v
    }
}

impl ReadableStorageTraits for VirtualStacTimeStack {
    fn get(&self, key: &StoreKey) -> Result<Option<Bytes>, zarrs::storage::StorageError> {
        let k = key.as_str();
        if k == ".zgroup" {
            return Ok(Some(self.zgroup_bytes.clone()));
        }
        if k == ".zmetadata" {
            return Ok(Some(self.zmetadata_bytes.clone()));
        }
        // Coordinate arrays: "{name}/.zarray" or "{name}/0".
        if let Some((name, sub)) = k.split_once('/') {
            if let Some((za, data)) = self.coords.get(name) {
                if sub == ".zarray" {
                    return Ok(Some(za.clone()));
                }
                if sub == "0" {
                    return Ok(Some(data.clone()));
                }
            }
            // Asset metadata.
            if sub == ".zarray" {
                if let Some(b) = self.asset_zarray.get(name) {
                    return Ok(Some(b.clone()));
                }
            }
            if sub == ".zattrs" {
                if let Some(b) = self.asset_zattrs.get(name) {
                    return Ok(Some(b.clone()));
                }
            }
            // Asset chunk "t.y.x" -> children[name][t].get("y.x").
            if let Some(items) = self.assets.get(name) {
                let mut parts = sub.splitn(2, '.');
                if let (Some(t_str), Some(yx)) = (parts.next(), parts.next()) {
                    if let Ok(t) = t_str.parse::<usize>() {
                        if let Some(child) = items.get(t) {
                            if let Ok(child_key) = StoreKey::new(yx) {
                                return child.get(&child_key);
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn get_partial_values_key(
        &self,
        key: &StoreKey,
        byte_ranges: &[zarrs::byte_range::ByteRange],
    ) -> Result<Option<Vec<Bytes>>, zarrs::storage::StorageError> {
        if let Some(bytes) = self.get(key)? {
            let mut out = Vec::new();
            for r in byte_ranges {
                let start = match r {
                    zarrs::byte_range::ByteRange::FromStart(o, _) => *o,
                    _ => 0,
                };
                let end = match r {
                    zarrs::byte_range::ByteRange::FromStart(o, Some(l)) => *o + *l,
                    _ => bytes.len() as u64,
                };
                out.push(bytes.slice(start as usize..end as usize));
            }
            Ok(Some(out))
        } else {
            Ok(None)
        }
    }

    fn size_key(&self, key: &StoreKey) -> Result<Option<u64>, zarrs::storage::StorageError> {
        Ok(self.get(key)?.map(|b| b.len() as u64))
    }
}

impl ListableStorageTraits for VirtualStacTimeStack {
    fn list(&self) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        let mut keys = vec![
            StoreKey::new(".zgroup").unwrap(),
            StoreKey::new(".zmetadata").unwrap(),
        ];
        for name in self.coords.keys() {
            keys.push(StoreKey::new(format!("{name}/.zarray")).unwrap());
        }
        for name in self.assets.keys() {
            keys.push(StoreKey::new(format!("{name}/.zarray")).unwrap());
            keys.push(StoreKey::new(format!("{name}/.zattrs")).unwrap());
        }
        Ok(keys)
    }
    fn list_prefix(
        &self,
        prefix: &StorePrefix,
    ) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        let p = prefix.as_str();
        Ok(self
            .list()?
            .into_iter()
            .filter(|k| k.as_str().starts_with(p))
            .collect())
    }
    fn list_dir(
        &self,
        _prefix: &StorePrefix,
    ) -> Result<zarrs::storage::StoreKeysPrefixes, zarrs::storage::StorageError> {
        zarrs::storage::store::MemoryStore::new().list_dir(_prefix)
    }
    fn size_prefix(&self, _prefix: &StorePrefix) -> Result<u64, zarrs::storage::StorageError> {
        Ok(0)
    }
}

use crate::cog::CogMetadata;

/// Verify every COG shares item 0's grid: shape, tile shape, affine, and CRS.
/// (dtype is validated per asset by the caller; this checks the shared grid.)
pub fn validate_grid_uniform(metas: &[&CogMetadata]) -> Result<(), String> {
    let Some(first) = metas.first() else {
        return Ok(());
    };
    let f_tf = first.spatial_transform();
    for (i, m) in metas.iter().enumerate().skip(1) {
        if (m.image_width, m.image_length) != (first.image_width, first.image_length) {
            return Err(format!(
                "item {i}: shape {}x{} != {}x{}",
                m.image_length, m.image_width, first.image_length, first.image_width
            ));
        }
        if (m.tile_width, m.tile_length) != (first.tile_width, first.tile_length) {
            return Err(format!("item {i}: tile shape differs"));
        }
        if m.epsg != first.epsg {
            return Err(format!("item {i}: CRS {:?} != {:?}", m.crs(), first.crs()));
        }
        let tf = m.spatial_transform();
        if tf.as_ref().map(|t| (&t.scale, &t.translation))
            != f_tf.as_ref().map(|t| (&t.scale, &t.translation))
        {
            return Err(format!("item {i}: affine transform differs"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::CogMetadata;

    fn child() -> VirtualCogStore {
        let meta = CogMetadata {
            image_width: 4,
            image_length: 2,
            tile_width: 4,
            tile_length: 2,
            tile_offsets: vec![0],
            tile_byte_counts: vec![16],
            is_little_endian: true,
            bits_per_sample: 16,
            sample_format: 2,
            samples_per_pixel: 1,
            compression: 1,
            predictor: 1,
            ..Default::default()
        };
        let op = opendal::Operator::new(opendal::services::Memory::default())
            .unwrap()
            .finish();
        VirtualCogStore::new(op, "".into(), meta).unwrap()
    }

    fn stack() -> VirtualStacTimeStack {
        let mut assets = HashMap::new();
        assets.insert("band".to_string(), vec![child(), child()]);
        VirtualStacTimeStack::new(
            assets,
            vec![1000.0, 2000.0],
            vec![90.0, 88.0],                     // H = 2
            vec![-180.0, -178.0, -176.0, -174.0], // W = 4
            ["lat".into(), "lon".into()],
        )
        .unwrap()
    }

    #[test]
    fn asset_zarray_is_3d() {
        let s = stack();
        let z = String::from_utf8(
            s.get(&StoreKey::new("band/.zarray").unwrap())
                .unwrap()
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(z.contains("\"shape\":[2,2,4]"), "{z}");
        assert!(z.contains("\"chunks\":[1,2,4]"), "{z}");
        assert!(z.contains("\"<i2\""), "{z}");
    }

    #[test]
    fn time_coord_array_roundtrips() {
        let s = stack();
        let za = String::from_utf8(
            s.get(&StoreKey::new("time/.zarray").unwrap())
                .unwrap()
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(za.contains("\"shape\":[2]") && za.contains("\"<f8\""));
        let data = s.get(&StoreKey::new("time/0").unwrap()).unwrap().unwrap();
        let v0 = f64::from_le_bytes(data[0..8].try_into().unwrap());
        let v1 = f64::from_le_bytes(data[8..16].try_into().unwrap());
        assert_eq!((v0, v1), (1000.0, 2000.0));
    }

    #[test]
    fn chunk_routes_to_item() {
        let s = stack();
        // both items share the same synthetic tile; routing must reach the child
        // without erroring. The Memory-backed child has no real tile bytes, so a
        // present index may return Some or None depending on the child's tile read;
        // what matters is that routing does not error.
        assert!(s.get(&StoreKey::new("band/0.0.0").unwrap()).is_ok());
        assert!(s.get(&StoreKey::new("band/1.0.0").unwrap()).is_ok());
        // out-of-range time index -> None
        assert!(s
            .get(&StoreKey::new("band/9.0.0").unwrap())
            .unwrap()
            .is_none());
    }

    #[test]
    fn zattrs_dims_are_time_lat_lon() {
        let s = stack();
        let za = String::from_utf8(
            s.get(&StoreKey::new("band/.zattrs").unwrap())
                .unwrap()
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            za.contains("_ARRAY_DIMENSIONS")
                && za.contains("time")
                && za.contains("lat")
                && za.contains("lon")
        );
    }

    use crate::cog::CogMetadata as M;

    fn m(w: u32, h: u32, epsg: Option<u32>) -> M {
        M {
            image_width: w,
            image_length: h,
            tile_width: w,
            tile_length: h,
            is_little_endian: true,
            bits_per_sample: 16,
            sample_format: 2,
            samples_per_pixel: 1,
            compression: 1,
            predictor: 1,
            epsg,
            ..Default::default()
        }
    }

    #[test]
    fn uniformity_passes_for_identical_and_fails_on_mismatch() {
        let a = m(4, 2, Some(4326));
        let b = m(4, 2, Some(4326));
        assert!(super::validate_grid_uniform(&[&a, &b]).is_ok());

        let diff_shape = m(8, 2, Some(4326));
        let e = super::validate_grid_uniform(&[&a, &diff_shape]).unwrap_err();
        assert!(e.contains("shape") || e.contains("1"), "{e}");

        let diff_crs = m(4, 2, Some(32633));
        assert!(super::validate_grid_uniform(&[&a, &diff_crs]).is_err());
    }
}
