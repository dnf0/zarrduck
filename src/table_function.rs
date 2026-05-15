use duckdb::vtab::{BindInfo, InitInfo, VTab};
use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeId, LogicalTypeHandle};
use duckdb::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zarrs::storage::store::FilesystemStore;
use zarrs::array::Array;

pub struct ReadZarrBindData {
    path: String,
}

pub struct ReadZarrInitData {
    done: AtomicBool,
}

pub struct ReadZarrVTab;

impl VTab for ReadZarrVTab {
    type InitData = ReadZarrInitData;
    type BindData = ReadZarrBindData;

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        if bind.get_parameter_count() < 1 {
            return Err("read_zarr requires at least 1 parameter (path)".into());
        }
        
        let path = bind.get_parameter(0).to_string();
        
        let store = FilesystemStore::new(&path).map_err(|e| format!("zarrs error: {}", e))?;
        let _array = Array::open(Arc::new(store), "/").map_err(|e| format!("zarrs error (array): {}", e))?;
        
        bind.add_result_column("path", LogicalTypeId::Varchar.into());
        Ok(ReadZarrBindData { path })
    }

    fn init(_init: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(ReadZarrInitData { done: AtomicBool::new(false) })
    }

    fn func(func: &duckdb::vtab::TableFunctionInfo<ReadZarrVTab>, output: &mut DataChunkHandle) -> Result<(), Box<dyn std::error::Error>> {
        let bind_data = func.get_bind_data();
        let init_data = func.get_init_data();

        if init_data.done.swap(true, Ordering::SeqCst) {
            output.set_len(0);
            return Ok(());
        }

        let vector = output.flat_vector(0);
        vector.insert(0, bind_data.path.as_str());
        
        output.set_len(1);
        Ok(())
    }
}
