use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId};
use duckdb::vtab::{BindInfo, InitInfo, VTab};
use duckdb::Result;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zarrs::storage::store::FilesystemStore;

pub struct MetadataBindData {
    pub shape: String,
    pub chunk_shape: String,
    pub data_type: String,
    pub crs: String,
}

pub struct MetadataInitData {
    pub done: AtomicBool,
}

pub struct ReadZarrMetadataVTab;

impl VTab for ReadZarrMetadataVTab {
    type InitData = MetadataInitData;
    type BindData = MetadataBindData;

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        let path = bind.get_parameter(0).to_string();
        let store = Arc::new(FilesystemStore::new(path).map_err(|e| e.to_string())?);
        let array = zarrs::array::Array::open(store, "/").map_err(|e| e.to_string())?;

        let shape = format!("{:?}", array.shape());
        let chunk_shape = format!("{:?}", array.chunk_grid().chunk_shape(&vec![0; array.shape().len()], array.shape()).unwrap_or(None));
        let data_type = format!("{:?}", array.data_type());

        let mut crs = "UNKNOWN".to_string();
        let metadata = array.metadata();
        if let zarrs::array::ArrayMetadata::V2(meta) = metadata {
            // Note: Use serde_json::Value::Object(meta.attributes.clone()) here!
            if let Some(geozarr) = crate::metadata::parse_geozarr_metadata(&serde_json::Value::Object(meta.attributes.clone())) {
                if let Some(c) = geozarr.crs { crs = c; }
            }
        } else if let zarrs::array::ArrayMetadata::V3(meta) = metadata {
            // Note: Use serde_json::Value::Object(meta.attributes.clone()) here!
            if let Some(geozarr) = crate::metadata::parse_geozarr_metadata(&serde_json::Value::Object(meta.attributes.clone())) {
                if let Some(c) = geozarr.crs { crs = c; }
            }
        }

        bind.add_result_column("array_shape", LogicalTypeId::Varchar.into());
        bind.add_result_column("chunk_shape", LogicalTypeId::Varchar.into());
        bind.add_result_column("data_type", LogicalTypeId::Varchar.into());
        bind.add_result_column("crs", LogicalTypeId::Varchar.into());

        Ok(MetadataBindData { shape, chunk_shape, data_type, crs })
    }

    fn init(_init: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(MetadataInitData { done: AtomicBool::new(false) })
    }

    fn func(func: &duckdb::vtab::TableFunctionInfo<ReadZarrMetadataVTab>, output: &mut DataChunkHandle) -> Result<(), Box<dyn std::error::Error>> {
        let init_data = func.get_init_data();
        if init_data.done.load(Ordering::Relaxed) {
            output.set_len(0);
            return Ok(());
        }

        let bind_data = func.get_bind_data();

        output.flat_vector(0).insert(0, bind_data.shape.as_str());
        output.flat_vector(1).insert(0, bind_data.chunk_shape.as_str());
        output.flat_vector(2).insert(0, bind_data.data_type.as_str());
        output.flat_vector(3).insert(0, bind_data.crs.as_str());
        output.set_len(1);

        init_data.done.store(true, Ordering::Relaxed);

        Ok(())
    }
}
