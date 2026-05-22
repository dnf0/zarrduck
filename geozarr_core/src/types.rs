use zarrs::array::DataType;

pub fn bytes_per_element(data_type: &DataType) -> u64 {
    match data_type {
        DataType::Float64 | DataType::Int64 | DataType::UInt64 => 8,
        DataType::Float32 | DataType::Int32 | DataType::UInt32 => 4,
        DataType::Int16 | DataType::UInt16 => 2,
        DataType::String => 64,
        _ => 1,
    }
}

pub fn string_to_zarr_type(duckdb_type: &str) -> Result<DataType, String> {
    match duckdb_type {
        "BOOLEAN" => Ok(DataType::Bool),
        "TINYINT" => Ok(DataType::Int8),
        "SMALLINT" => Ok(DataType::Int16),
        "INTEGER" => Ok(DataType::Int32),
        "BIGINT" => Ok(DataType::Int64),
        "UTINYINT" => Ok(DataType::UInt8),
        "USMALLINT" => Ok(DataType::UInt16),
        "UINTEGER" => Ok(DataType::UInt32),
        "UBIGINT" => Ok(DataType::UInt64),
        "FLOAT" | "REAL" => Ok(DataType::Float32),
        "DOUBLE" | "FLOAT8" | "DECIMAL" | "NUMERIC" => Ok(DataType::Float64),
        "VARCHAR" => Ok(DataType::String),
        _ => Err(format!("Unsupported DuckDB type: {}", duckdb_type)),
    }
}

pub enum ChunkBuffer {
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    String(Vec<String>),
    Bool(Vec<bool>),
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    UInt8(Vec<u8>),
    UInt16(Vec<u16>),
    UInt32(Vec<u32>),
    UInt64(Vec<u64>),
}

#[macro_export]
macro_rules! dispatch_zarr_type {
    ($data_type:expr, $mac:ident, $($args:tt)*) => {
        match $data_type {
            zarrs::array::DataType::Float32 => $mac!(f32, $crate::types::ChunkBuffer::Float32, $($args)*),
            zarrs::array::DataType::Float64 => $mac!(f64, $crate::types::ChunkBuffer::Float64, $($args)*),
            zarrs::array::DataType::Int32 => $mac!(i32, $crate::types::ChunkBuffer::Int32, $($args)*),
            zarrs::array::DataType::Int64 => $mac!(i64, $crate::types::ChunkBuffer::Int64, $($args)*),
            zarrs::array::DataType::String => $mac!(String, $crate::types::ChunkBuffer::String, $($args)*),
            zarrs::array::DataType::Bool => $mac!(bool, $crate::types::ChunkBuffer::Bool, $($args)*),
            zarrs::array::DataType::Int8 => $mac!(i8, $crate::types::ChunkBuffer::Int8, $($args)*),
            zarrs::array::DataType::Int16 => $mac!(i16, $crate::types::ChunkBuffer::Int16, $($args)*),
            zarrs::array::DataType::UInt8 => $mac!(u8, $crate::types::ChunkBuffer::UInt8, $($args)*),
            zarrs::array::DataType::UInt16 => $mac!(u16, $crate::types::ChunkBuffer::UInt16, $($args)*),
            zarrs::array::DataType::UInt32 => $mac!(u32, $crate::types::ChunkBuffer::UInt32, $($args)*),
            zarrs::array::DataType::UInt64 => $mac!(u64, $crate::types::ChunkBuffer::UInt64, $($args)*),
            _ => panic!("Unsupported type: {:?}", $data_type),
        }
    }
}
