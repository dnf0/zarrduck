use std::io::Read;
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
        Ok(Self { base_url: base_url.trim_end_matches('/').to_string() })
    }

    fn key_to_url(&self, key: &StoreKey) -> String {
        let k = key.as_str().trim_start_matches('/');
        if k.is_empty() { self.base_url.clone() } else { format!("{}/{}", self.base_url, k) }
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
        Ok(self.fetch(&self.key_to_url(key), None)?
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
                ByteRange::FromStart(o, None)    => format!("bytes={}-", o),
                ByteRange::FromEnd(o, Some(l))   => format!("bytes=-{}", o + l),
                ByteRange::FromEnd(o, None)      => format!("bytes=-{}", o),
            };
            match self.fetch(&url, Some(&header))? {
                Some(b) => out.push(zarrs::storage::Bytes::from(b)),
                None    => return Ok(None),
            }
        }
        Ok(Some(out))
    }

    fn size_key(&self, key: &StoreKey) -> Result<Option<u64>, StorageError> {
        AGENT.with(|agent| {
            match agent.head(&self.key_to_url(key)).call() {
                Ok(resp) => Ok(resp.header("content-length").and_then(|v| v.parse().ok())),
                Err(ureq::Error::Status(404, _)) => Ok(None),
                Err(e) => Err(StorageError::Other(e.to_string())),
            }
        })
    }
}

impl zarrs::storage::ListableStorageTraits for SyncHttpStore {
    fn list(&self) -> Result<zarrs::storage::StoreKeys, StorageError> {
        Err(StorageError::Other("list not supported".into()))
    }
    fn list_prefix(&self, _: &zarrs::storage::StorePrefix) -> Result<zarrs::storage::StoreKeys, StorageError> {
        Err(StorageError::Other("list_prefix not supported".into()))
    }
    fn list_dir(&self, _: &zarrs::storage::StorePrefix) -> Result<zarrs::storage::StoreKeysPrefixes, StorageError> {
        Err(StorageError::Other("list_dir not supported".into()))
    }
    fn size_prefix(&self, _: &zarrs::storage::StorePrefix) -> Result<u64, StorageError> {
        Err(StorageError::Other("size_prefix not supported".into()))
    }
}
