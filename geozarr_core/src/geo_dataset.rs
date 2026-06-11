use crate::query_planner::QueryConstraints;
use crate::types::ChunkBuffer;
use std::fmt;
use zarrs::array::DataType;

/// Represents errors that can occur during dataset operations.
#[derive(Debug)]
pub enum GeoDatasetError {
    /// Failed to read or parse the schema.
    Schema(String),
    /// Failed to plan or execute a scan.
    Scan(String),
    /// Failed to read a chunk.
    ChunkRead(String),
    /// An unknown or generic error.
    Other(String),
}

impl fmt::Display for GeoDatasetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeoDatasetError::Schema(msg) => write!(f, "Schema error: {}", msg),
            GeoDatasetError::Scan(msg) => write!(f, "Scan error: {}", msg),
            GeoDatasetError::ChunkRead(msg) => write!(f, "Chunk read error: {}", msg),
            GeoDatasetError::Other(msg) => write!(f, "Other error: {}", msg),
        }
    }
}

impl std::error::Error for GeoDatasetError {}

/// Indicates the result of a chunk read operation.
#[derive(Debug, Clone)]
pub enum ChunkReadStatus {
    /// Data was successfully read into the buffer, with spatial subset information.
    Read(crate::scanner::SubsetInfo),
    /// The stream has been exhausted and no more data is available.
    Exhausted,
}

/// An abstraction for spatial datasets that can be scanned.
pub trait GeoDataset: Send + Sync {
    /// Returns the schema of the dataset.
    fn schema(&self) -> Result<Vec<(String, DataType)>, GeoDatasetError>;
    
    /// Prepares a scan over the dataset based on the provided query constraints.
    fn scan(
        &self, 
        constraints: &QueryConstraints
    ) -> Result<Box<dyn ChunkStream>, GeoDatasetError>;
}

/// A stream of chunks resulting from a dataset scan.
pub trait ChunkStream: Send + Sync {
    /// Returns the estimated number of chunks in this stream, if known.
    fn estimated_chunks(&self) -> Option<u64>;

    /// Reads a chunk into the provided buffer.
    /// 
    /// Returns `ChunkReadStatus::Read` if data was read, or `ChunkReadStatus::Exhausted` if there are no more chunks.
    fn read_chunk(
        &self, 
        chunk_idx: u64, 
        buffer: &mut ChunkBuffer
    ) -> Result<ChunkReadStatus, GeoDatasetError>;
}
