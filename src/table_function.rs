use duckdb::vtab::{BindInfo, InitInfo, VTab};
use duckdb::core::{DataChunkHandle, Inserter, LogicalTypeId};
use duckdb::Result;
use std::sync::atomic::{AtomicBool, Ordering};

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

    fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        bind.add_result_column("path", LogicalTypeId::Varchar.into());
        let path = bind.get_parameter(0).to_string();
        Ok(ReadZarrBindData { path })
    }

    fn init(_init: &InitInfo) -> Result<Self::InitData, Box<dyn std::error::Error>> {
        Ok(ReadZarrInitData { done: AtomicBool::new(false) })
    }

    fn func(func: &duckdb::vtab::TableFunctionInfo<ReadZarrVTab>, output: &mut DataChunkHandle) -> Result<(), Box<dyn std::error::Error>> {
        let bind_data = func.get_bind_data();
        let init_data = func.get_init_data();

        if init_data.done.load(Ordering::Relaxed) {
            output.set_len(0);
            return Ok(());
        }

        let vector = output.flat_vector(0);
        vector.insert(0, bind_data.path.as_str());
        
        output.set_len(1);
        init_data.done.store(true, Ordering::Relaxed);
        Ok(())
    }
}
