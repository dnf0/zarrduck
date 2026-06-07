use crate::virtual_store::VirtualCogStore;
use bytes::Bytes;
use std::collections::HashMap;
use zarrs::storage::{ListableStorageTraits, ReadableStorageTraits, StoreKey, StorePrefix};

pub struct VirtualStacStore {
    children: HashMap<String, VirtualCogStore>,
    zgroup_bytes: Bytes,
    zmetadata_bytes: Bytes,
}

impl VirtualStacStore {
    pub fn new(children: HashMap<String, VirtualCogStore>) -> Self {
        let zgroup_bytes = Bytes::from(r#"{"zarr_format": 2}"#);

        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert(".zgroup".to_string(), serde_json::json!({"zarr_format": 2}));

        for (name, child) in &children {
            // Get the child's .zarray json
            if let Ok(Some(bytes)) = child.get(&StoreKey::new(".zarray").unwrap()) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    metadata_map.insert(format!("{}/.zarray", name), json);
                }
            }
            if let Ok(Some(bytes)) = child.get(&StoreKey::new(".zattrs").unwrap()) {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    metadata_map.insert(format!("{}/.zattrs", name), json);
                }
            }
        }

        let zmetadata_json = serde_json::json!({
            "metadata": metadata_map,
            "zarr_consolidated_format": 1
        });

        let zmetadata_bytes = Bytes::from(zmetadata_json.to_string());

        Self {
            children,
            zgroup_bytes,
            zmetadata_bytes,
        }
    }
}

impl ReadableStorageTraits for VirtualStacStore {
    fn get(&self, key: &StoreKey) -> Result<Option<Bytes>, zarrs::storage::StorageError> {
        let key_str = key.as_str();
        if key_str == ".zgroup" {
            return Ok(Some(self.zgroup_bytes.clone()));
        }
        if key_str == ".zmetadata" {
            return Ok(Some(self.zmetadata_bytes.clone()));
        }

        // Delegate to child
        if let Some(slash_idx) = key_str.find('/') {
            let child_name = &key_str[..slash_idx];
            let child_key_str = &key_str[slash_idx + 1..];
            if let Some(child) = self.children.get(child_name) {
                if let Ok(child_key) = StoreKey::new(child_key_str) {
                    return child.get(&child_key);
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
        let key_str = key.as_str();
        if key_str == ".zgroup" || key_str == ".zmetadata" {
            return Err(zarrs::storage::StorageError::Other(
                "partial read not supported on metadata".into(),
            ));
        }

        if let Some(slash_idx) = key_str.find('/') {
            let child_name = &key_str[..slash_idx];
            let child_key_str = &key_str[slash_idx + 1..];
            if let Some(child) = self.children.get(child_name) {
                if let Ok(child_key) = StoreKey::new(child_key_str) {
                    return child.get_partial_values_key(&child_key, byte_ranges);
                }
            }
        }
        Ok(None)
    }

    fn size_key(&self, key: &StoreKey) -> Result<Option<u64>, zarrs::storage::StorageError> {
        let key_str = key.as_str();
        if key_str == ".zgroup" {
            return Ok(Some(self.zgroup_bytes.len() as u64));
        }
        if key_str == ".zmetadata" {
            return Ok(Some(self.zmetadata_bytes.len() as u64));
        }

        if let Some(slash_idx) = key_str.find('/') {
            let child_name = &key_str[..slash_idx];
            let child_key_str = &key_str[slash_idx + 1..];
            if let Some(child) = self.children.get(child_name) {
                if let Ok(child_key) = StoreKey::new(child_key_str) {
                    return child.size_key(&child_key);
                }
            }
        }
        Ok(None)
    }
}

impl ListableStorageTraits for VirtualStacStore {
    fn list(&self) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        let mut keys = vec![
            StoreKey::new(".zgroup").unwrap(),
            StoreKey::new(".zmetadata").unwrap(),
        ];
        for name in self.children.keys() {
            keys.push(StoreKey::new(format!("{}/.zarray", name)).unwrap());
            keys.push(StoreKey::new(format!("{}/.zattrs", name)).unwrap());
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
        // Group discovery uses the consolidated `.zmetadata`; a precise directory
        // listing isn't needed. zarrs 0.16.4 exposes no public `StoreKeysPrefixes`
        // constructor, so derive an empty value from an empty in-memory store.
        let empty = zarrs::storage::store::MemoryStore::new();
        empty.list_dir(_prefix)
    }
    fn size_prefix(&self, _prefix: &StorePrefix) -> Result<u64, zarrs::storage::StorageError> {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cog::CogMetadata;

    fn child() -> VirtualCogStore {
        let mut meta = CogMetadata {
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
        meta.pixel_scale = vec![2.0, 2.0, 0.0];
        meta.tiepoint = vec![0.0, 0.0, 0.0, -180.0, 90.0, 0.0];
        meta.epsg = Some(4326);
        let op = opendal::Operator::new(opendal::services::Memory::default())
            .unwrap()
            .finish();
        VirtualCogStore::new(op, "".to_string(), meta).unwrap()
    }

    #[test]
    fn zmetadata_includes_child_zattrs() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let zmeta = String::from_utf8(
            store
                .get(&StoreKey::new(".zmetadata").unwrap())
                .unwrap()
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(zmeta.contains("band/.zarray"));
        assert!(
            zmeta.contains("band/.zattrs"),
            "group metadata must carry child .zattrs: {zmeta}"
        );
        assert!(zmeta.contains("spatial_transform"));
    }

    #[test]
    fn routes_child_zattrs_key() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let za = store.get(&StoreKey::new("band/.zattrs").unwrap()).unwrap();
        assert!(za.is_some(), "band/.zattrs must route to the child");
    }

    #[test]
    fn list_returns_child_keys_not_error() {
        let mut m = HashMap::new();
        m.insert("band".to_string(), child());
        let store = VirtualStacStore::new(m);
        let keys = store.list().expect("list must succeed");
        let set: Vec<String> = keys.iter().map(|k| k.as_str().to_string()).collect();
        assert!(set.iter().any(|k| k == "band/.zarray"));
        assert!(set.iter().any(|k| k == "band/.zattrs"));
    }
}
