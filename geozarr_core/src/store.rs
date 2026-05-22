use std::io::Read;
use std::sync::Arc;
use zarrs::byte_range::ByteRange;
use zarrs::storage::{MaybeBytes, ReadableStorageTraits, StorageError, StoreKey};

thread_local! {
    static AGENT: ureq::Agent = ureq::AgentBuilder::new()
        .timeout_read(std::time::Duration::from_secs(600))
        .timeout_write(std::time::Duration::from_secs(30))
        .build();
}

/// Synchronous HTTP store backed by ureq.
///
/// ureq uses plain blocking OS sockets with no async runtime. Each DuckDB
/// worker thread gets its own thread-local agent — independent TCP connections,
/// no shared state, no tokio contention when fetching large chunks in parallel.
pub struct SyncHttpStore {
    base_url: String,
}

impl SyncHttpStore {
    pub fn new(base_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    fn key_to_url(&self, key: &StoreKey) -> String {
        let k = key.as_str().trim_start_matches('/');
        if k.is_empty() {
            self.base_url.clone()
        } else {
            format!("{}/{}", self.base_url, k)
        }
    }

    fn fetch(&self, url: &str, range: Option<&str>) -> Result<Option<Vec<u8>>, StorageError> {
        AGENT.with(|agent| {
            let mut req = agent.get(url);
            if let Some(r) = range {
                req = req.set("Range", r);
            }
            match req.call() {
                Ok(resp) => {
                    let mut buf = Vec::new();
                    resp.into_reader()
                        .read_to_end(&mut buf)
                        .map_err(|e| StorageError::Other(e.to_string()))?;
                    Ok(Some(buf))
                }
                Err(ureq::Error::Status(404, _)) => Ok(None),
                Err(e) => Err(StorageError::Other(e.to_string())),
            }
        })
    }
}

impl ReadableStorageTraits for SyncHttpStore {
    fn get(&self, key: &StoreKey) -> Result<MaybeBytes, StorageError> {
        Ok(self
            .fetch(&self.key_to_url(key), None)?
            .map(zarrs::storage::Bytes::from))
    }

    fn get_partial_values_key(
        &self,
        key: &StoreKey,
        byte_ranges: &[ByteRange],
    ) -> Result<Option<Vec<zarrs::storage::Bytes>>, StorageError> {
        let url = self.key_to_url(key);
        let mut out = Vec::with_capacity(byte_ranges.len());
        for range in byte_ranges {
            let header = match range {
                ByteRange::FromStart(o, Some(l)) => format!("bytes={}-{}", o, o + l - 1),
                ByteRange::FromStart(o, None) => format!("bytes={}-", o),
                ByteRange::FromEnd(o, Some(l)) => format!("bytes=-{}", o + l),
                ByteRange::FromEnd(o, None) => format!("bytes=-{}", o),
            };
            match self.fetch(&url, Some(&header))? {
                Some(b) => out.push(zarrs::storage::Bytes::from(b)),
                None => return Ok(None),
            }
        }
        Ok(Some(out))
    }

    fn size_key(&self, key: &StoreKey) -> Result<Option<u64>, StorageError> {
        AGENT.with(|agent| match agent.head(&self.key_to_url(key)).call() {
            Ok(resp) => Ok(resp.header("content-length").and_then(|v| v.parse().ok())),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(StorageError::Other(e.to_string())),
        })
    }
}

impl zarrs::storage::ListableStorageTraits for SyncHttpStore {
    fn list(&self) -> Result<zarrs::storage::StoreKeys, StorageError> {
        Err(StorageError::Other("list not supported".into()))
    }
    fn list_prefix(
        &self,
        _: &zarrs::storage::StorePrefix,
    ) -> Result<zarrs::storage::StoreKeys, StorageError> {
        Err(StorageError::Other("list_prefix not supported".into()))
    }
    fn list_dir(
        &self,
        _: &zarrs::storage::StorePrefix,
    ) -> Result<zarrs::storage::StoreKeysPrefixes, StorageError> {
        Err(StorageError::Other("list_dir not supported".into()))
    }
    fn size_prefix(&self, _: &zarrs::storage::StorePrefix) -> Result<u64, StorageError> {
        Err(StorageError::Other("size_prefix not supported".into()))
    }
}

pub struct ResolvedStore {
    pub store: Arc<dyn ReadableStorageTraits>,
    pub is_remote: bool,
}

pub fn resolve_sync_store(
    path: &str,
) -> std::result::Result<ResolvedStore, Box<dyn std::error::Error>> {
    if path.starts_with("s3://")
        || path.starts_with("abfs://")
        || path.starts_with("http://")
        || path.starts_with("https://")
    {
        let http_url = if path.starts_with("s3://") {
            let bucket_and_path = path.strip_prefix("s3://").unwrap();
            let mut parts = bucket_and_path.splitn(2, '/');
            let bucket = parts.next().unwrap();
            let rest = parts.next().unwrap_or("");
            format!("https://{}.s3.amazonaws.com/{}", bucket, rest)
        } else if path.starts_with("abfs://") {
            let bucket_and_path = path.strip_prefix("abfs://").unwrap();
            let mut parts = bucket_and_path.splitn(2, '/');
            let bucket = parts.next().unwrap();
            let rest = parts.next().unwrap_or("");
            format!(
                "https://cpdataeuwest.blob.core.windows.net/{}/{}",
                bucket, rest
            )
        } else {
            path.to_string()
        };

        Ok(ResolvedStore {
            store: Arc::new(SyncHttpStore::new(&http_url)?),
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
