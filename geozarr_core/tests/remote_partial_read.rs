//! Regression tests for ranged partial reads through the opendal adapter.
//!
//! These exercise `AsyncToSyncOpendalStore::get_partial_values_key` against a
//! Zarr v3 *sharded* array whose shard index lives at the END of each shard
//! (`ShardingIndexLocation::End` — the spec default that `xarray.to_zarr`
//! writes). The sharding partial decoder requests the shard index as a suffix
//! (`ByteRange::FromEnd`), so an adapter that does not honor `FromEnd` either
//! corrupts the crc32c-checked index (correctness) or over-fetches whole shards
//! (performance).
//!
//! The array is *written* with zarrs' native `FilesystemStore`, but *read back*
//! exclusively through an opendal `Fs` operator wrapped in
//! `AsyncToSyncOpendalStore` — the bug lives in that adapter, so the read path
//! must traverse it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use geozarr_core::store::AsyncToSyncOpendalStore;
use zarrs::array::codec::array_to_bytes::sharding::{ShardingCodecBuilder, ShardingIndexLocation};
use zarrs::array::{Array, ArrayBuilder, DataType, FillValue};
use zarrs::array_subset::ArraySubset;
use zarrs::storage::store::FilesystemStore;
use zarrs::storage::ReadableStorageTraits;

const ARRAY_PATH: &str = "/array";
// 16x16 array, 8x8 shards, each shard holding a 4x4 grid of 2x2 inner chunks.
const ARRAY_SHAPE: [u64; 2] = [16, 16];
const SHARD_SHAPE: [u64; 2] = [8, 8];
const INNER_CHUNK_SHAPE: [u64; 2] = [2, 2];

/// Write a unique temp dir under the system temp area (avoids a `tempfile`
/// dev-dependency, keeping the staged change set to store.rs + this test).
fn unique_tempdir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "geozarr_partial_read_{tag}_{}_{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build value `row * 1000 + col` so every cell is uniquely identifiable.
fn cell_value(row: u64, col: u64) -> i32 {
    (row * 1000 + col) as i32
}

/// Write a sharded v3 array with the shard index at `index_location` into `dir`,
/// filling every cell with `cell_value`.
fn write_sharded_array(dir: &std::path::Path, index_location: ShardingIndexLocation) {
    let store: Arc<FilesystemStore> = Arc::new(FilesystemStore::new(dir).unwrap());

    let mut sharding_builder =
        ShardingCodecBuilder::new(INNER_CHUNK_SHAPE.as_slice().try_into().unwrap());
    sharding_builder.index_location(index_location);

    let array = ArrayBuilder::new(
        ARRAY_SHAPE.to_vec(),
        DataType::Int32,
        SHARD_SHAPE.to_vec().try_into().unwrap(),
        FillValue::from(-1i32),
    )
    .array_to_bytes_codec(Box::new(sharding_builder.build()))
    .dimension_names(["y", "x"].into())
    .build(store.clone(), ARRAY_PATH)
    .unwrap();
    array.store_metadata().unwrap();

    let full = ArraySubset::new_with_shape(ARRAY_SHAPE.to_vec());
    let mut values = Vec::with_capacity((ARRAY_SHAPE[0] * ARRAY_SHAPE[1]) as usize);
    for row in 0..ARRAY_SHAPE[0] {
        for col in 0..ARRAY_SHAPE[1] {
            values.push(cell_value(row, col));
        }
    }
    array
        .store_array_subset_elements::<i32>(&full, &values)
        .unwrap();
}

/// Open `dir` through an opendal `Fs` operator wrapped in the adapter under test.
fn open_via_adapter(dir: &std::path::Path) -> AsyncToSyncOpendalStore {
    let builder = opendal::services::Fs::default().root(dir.to_str().unwrap());
    let operator = opendal::Operator::new(builder).unwrap().finish();
    AsyncToSyncOpendalStore::new(operator)
}

/// Assert a sub-region read through the adapter yields exactly `cell_value`.
fn assert_subregion_correct<T: ReadableStorageTraits + 'static>(store: Arc<T>) {
    let array = Array::open(store, ARRAY_PATH).unwrap();
    // A 4x4 window straddling the boundary between shards [0,0] and [1,1].
    let subset = ArraySubset::new_with_ranges(&[6..10, 6..10]);
    let got = array
        .retrieve_array_subset_elements::<i32>(&subset)
        .unwrap();

    let mut expected = Vec::new();
    for row in 6..10 {
        for col in 6..10 {
            expected.push(cell_value(row, col));
        }
    }
    assert_eq!(
        got, expected,
        "sub-region values must match what was written"
    );
}

/// Task A (P0): end-indexed shards must read correctly through the adapter.
/// Before the fix this fails with "the checksum is invalid" because the adapter
/// returns the whole shard for the `FromEnd` shard-index suffix request.
#[test]
fn end_indexed_shard_subregion_reads_correctly() {
    let dir = unique_tempdir("end_correct");
    write_sharded_array(&dir, ShardingIndexLocation::End);
    let store = Arc::new(open_via_adapter(&dir));
    assert_subregion_correct(store);
    std::fs::remove_dir_all(&dir).ok();
}

/// Task D non-regression: start-indexed shards still read correctly.
#[test]
fn start_indexed_shard_subregion_reads_correctly() {
    let dir = unique_tempdir("start_correct");
    write_sharded_array(&dir, ShardingIndexLocation::Start);
    let store = Arc::new(open_via_adapter(&dir));
    assert_subregion_correct(store);
    std::fs::remove_dir_all(&dir).ok();
}

/// A wrapping store that delegates to `AsyncToSyncOpendalStore` and counts every
/// byte returned through the partial-read path, so a test can prove a windowed
/// sharded read fetches far fewer bytes than a whole shard.
struct CountingStore {
    inner: AsyncToSyncOpendalStore,
    /// Bytes fetched for the shard-data region (`FromStart` inner-chunk reads).
    data_bytes: AtomicU64,
    /// Bytes fetched for the shard index (the `FromEnd` suffix request).
    index_bytes: AtomicU64,
}

impl CountingStore {
    fn new(inner: AsyncToSyncOpendalStore) -> Self {
        Self {
            inner,
            data_bytes: AtomicU64::new(0),
            index_bytes: AtomicU64::new(0),
        }
    }
    fn data_bytes(&self) -> u64 {
        self.data_bytes.load(Ordering::SeqCst)
    }
    fn index_bytes(&self) -> u64 {
        self.index_bytes.load(Ordering::SeqCst)
    }
}

impl ReadableStorageTraits for CountingStore {
    fn get(
        &self,
        key: &zarrs::storage::StoreKey,
    ) -> Result<Option<bytes::Bytes>, zarrs::storage::StorageError> {
        self.inner.get(key)
    }

    fn get_partial_values_key(
        &self,
        key: &zarrs::storage::StoreKey,
        byte_ranges: &[zarrs::byte_range::ByteRange],
    ) -> Result<Option<Vec<bytes::Bytes>>, zarrs::storage::StorageError> {
        let res = self.inner.get_partial_values_key(key, byte_ranges)?;
        if let Some(parts) = &res {
            for (range, buf) in byte_ranges.iter().zip(parts) {
                let n = buf.len() as u64;
                match range {
                    // The shard index is requested as a suffix (`FromEnd`); all
                    // other reads target the shard-data region.
                    zarrs::byte_range::ByteRange::FromEnd(_, _) => {
                        self.index_bytes.fetch_add(n, Ordering::SeqCst);
                    }
                    zarrs::byte_range::ByteRange::FromStart(_, _) => {
                        self.data_bytes.fetch_add(n, Ordering::SeqCst);
                    }
                }
            }
        }
        Ok(res)
    }

    fn size_key(
        &self,
        key: &zarrs::storage::StoreKey,
    ) -> Result<Option<u64>, zarrs::storage::StorageError> {
        self.inner.size_key(key)
    }
}

/// Task C (P1): a windowed read of an end-indexed sharded array must fetch only
/// the touched inner chunk's data, not the whole shard. A shard holds a 4x4 grid
/// of 2x2 inner chunks = 8*8*4 = 256 data bytes; a 2x2 window touches exactly one
/// inner chunk (4 i32 = 16 bytes) plus the shard index suffix. Before the fix the
/// adapter returned the whole shard for *every* range, so the data region alone
/// was over-fetched ~16x; after the fix only the 16-byte inner chunk is read.
#[test]
fn windowed_read_fetches_far_less_than_whole_shard() {
    let dir = unique_tempdir("perf");
    write_sharded_array(&dir, ShardingIndexLocation::End);

    let counting = Arc::new(CountingStore::new(open_via_adapter(&dir)));
    let array = Array::open(counting.clone(), ARRAY_PATH).unwrap();

    // Read a single 2x2 inner chunk inside shard [0,0].
    let subset = ArraySubset::new_with_ranges(&[0..2, 0..2]);
    let got = array
        .retrieve_array_subset_elements::<i32>(&subset)
        .unwrap();
    assert_eq!(
        got,
        vec![
            cell_value(0, 0),
            cell_value(0, 1),
            cell_value(1, 0),
            cell_value(1, 1)
        ]
    );

    let whole_shard_data_bytes: u64 = SHARD_SHAPE[0] * SHARD_SHAPE[1] * 4; // 256
    let touched_inner_chunk_bytes: u64 = INNER_CHUNK_SHAPE[0] * INNER_CHUNK_SHAPE[1] * 4; // 16
    let data = counting.data_bytes();
    let index = counting.index_bytes();

    assert!(
        index > 0,
        "the shard index (FromEnd suffix) must have been fetched"
    );
    // The data region fetched must be just the touched inner chunk(s), far below
    // the full shard. Allow a little slack but well under the whole-shard size.
    assert!(
        data >= touched_inner_chunk_bytes && data < whole_shard_data_bytes / 4,
        "windowed read fetched {data} data bytes (index {index}); expected ~{touched_inner_chunk_bytes} \
         (one inner chunk), far below the whole shard's {whole_shard_data_bytes} data bytes"
    );

    std::fs::remove_dir_all(&dir).ok();
}
