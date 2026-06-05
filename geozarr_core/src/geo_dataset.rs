use crate::query_planner::QueryConstraints;
use zarrs::array::DataType;
use std::any::Any;

pub trait ScanPlan: Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

pub trait GeoDataset: Send + Sync {
    fn schema(&self) -> Result<Vec<(String, DataType)>, Box<dyn std::error::Error>>;
    fn plan_scan(&self, constraints: &QueryConstraints) -> Result<Box<dyn ScanPlan>, Box<dyn std::error::Error>>;
    fn num_chunks(&self, plan: &dyn ScanPlan) -> u64;
    // We will pass the exact thread index to let the dataset yield its rows
    // For now, minimal method definition. The actual macro integration might require specific return types.
    fn read_chunk(&self, plan: &dyn ScanPlan, chunk_idx: u64, output_buffer: &mut crate::types::ChunkBuffer) -> Result<(), Box<dyn std::error::Error>>;
}
