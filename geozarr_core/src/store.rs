use std::sync::Arc;
use zarrs::storage::ReadableStorageTraits;

pub struct ResolvedStore {
    pub store: Arc<dyn ReadableStorageTraits>,
    pub is_remote: bool,
}

pub fn resolve_sync_store(
    path: &str,
) -> std::result::Result<ResolvedStore, Box<dyn std::error::Error>> {
    if path.starts_with("s3://") {
        let bucket_and_path = path.strip_prefix("s3://").unwrap();
        let bucket = bucket_and_path.split('/').next().unwrap_or(bucket_and_path);
        let root = bucket_and_path.strip_prefix(bucket).unwrap_or("/");
        let builder = opendal::services::S3::default().bucket(bucket).root(root);
        let operator = opendal::Operator::new(builder)?.finish().blocking();
        Ok(ResolvedStore {
            store: Arc::new(zarrs::storage::store::OpendalStore::new(operator)),
            is_remote: true,
        })
    } else if path.starts_with("http://") || path.starts_with("https://") {
        let builder = opendal::services::Http::default().endpoint(path);
        let operator = opendal::Operator::new(builder)?.finish().blocking();
        Ok(ResolvedStore {
            store: Arc::new(zarrs::storage::store::OpendalStore::new(operator)),
            is_remote: true,
        })
    } else {
        let canonical_path =
            std::fs::canonicalize(path).map_err(|e| format!("Invalid path: {}", e))?;
        let allowed_dir = std::env::var("GEOZARR_ALLOW_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let allowed_canon = std::fs::canonicalize(&allowed_dir)
            .map_err(|e| format!("Invalid GEOZARR_ALLOW_PATH: {}", e))?;
        if !canonical_path.starts_with(allowed_canon) {
            return Err("Access denied. Path is not within the allowed sandbox directory (GEOZARR_ALLOW_PATH or CWD).".into());
        }
        let store = zarrs::storage::store::FilesystemStore::new(canonical_path)?;
        Ok(ResolvedStore {
            store: Arc::new(store),
            is_remote: false,
        })
    }
}

pub fn resolve_async_store(
    path: &str,
) -> std::result::Result<
    Arc<dyn zarrs::storage::AsyncWritableStorageTraits>,
    Box<dyn std::error::Error>,
> {
    if path.starts_with("s3://") {
        let bucket_and_path = path.strip_prefix("s3://").unwrap();
        let bucket = bucket_and_path.split('/').next().unwrap_or(bucket_and_path);
        let root = bucket_and_path.strip_prefix(bucket).unwrap_or("/");
        let builder = opendal::services::S3::default().bucket(bucket).root(root);
        let operator = opendal::Operator::new(builder)?.finish();
        Ok(Arc::new(zarrs::storage::store::AsyncOpendalStore::new(
            operator,
        )))
    } else {
        let builder = opendal::services::Fs::default().root(path);
        let operator = opendal::Operator::new(builder)?.finish();
        Ok(Arc::new(zarrs::storage::store::AsyncOpendalStore::new(
            operator,
        )))
    }
}

pub async fn list_arrays(uri: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let operator = if uri.starts_with("http") {
        opendal::Operator::new(opendal::services::Http::default().endpoint(uri))?.finish()
    } else {
        opendal::Operator::new(opendal::services::Fs::default().root(uri))?.finish()
    };

    let is_group = operator.is_exist(".zgroup").await.unwrap_or(false);
    let mut arrays = Vec::new();

    if is_group {
        // Try reading consolidated metadata first (crucial for HTTP where listing is unsupported)
        if let Ok(metadata_bytes) = operator.read(".zmetadata").await {
            if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes.to_bytes()) {
                if let Some(metadata) = metadata_json.get("metadata").and_then(|m| m.as_object()) {
                    let mut arrays_set = std::collections::HashSet::new();
                    for (key, _) in metadata {
                        if key.ends_with(".zarray") {
                            arrays_set.insert(key.strip_suffix("/.zarray").unwrap_or("").to_string());
                        }
                    }
                    if !arrays_set.is_empty() {
                        let mut sorted: Vec<_> = arrays_set.into_iter().collect();
                        sorted.sort();
                        return Ok(sorted);
                    }
                }
            }
        }

        let entries = operator.list("/").await?;
        for entry in entries {
            if entry.metadata().is_dir() {
                let path = entry.path();
                if operator
                    .is_exist(&format!("{}.zarray", path))
                    .await
                    .unwrap_or(false)
                {
                    arrays.push(path.trim_end_matches('/').to_string());
                }
            }
        }
    } else if operator.is_exist(".zarray").await.unwrap_or(false) {
        arrays.push("".to_string());
    }

    Ok(arrays)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_arrays() {
        let arrays = list_arrays("../climate_data.zarr").await.unwrap();
        println!("Found arrays: {:?}", arrays);
        // assert_eq!(arrays.len(), 4);
    }
}
