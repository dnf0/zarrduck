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
            return Err(zarrs::storage::StorageError::Other("partial read not supported on metadata".into()));
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
        Err(zarrs::storage::StorageError::Other("list not supported".into()))
    }
    fn list_prefix(&self, _prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeys, zarrs::storage::StorageError> {
        Err(zarrs::storage::StorageError::Other("list_prefix not supported".into()))
    }
    fn list_dir(&self, _prefix: &StorePrefix) -> Result<zarrs::storage::StoreKeysPrefixes, zarrs::storage::StorageError> {
        Err(zarrs::storage::StorageError::Other("list_dir not supported".into()))
    }
    fn size_prefix(&self, _prefix: &StorePrefix) -> Result<u64, zarrs::storage::StorageError> {
        Err(zarrs::storage::StorageError::Other("size_prefix not supported".into()))
    }
}
