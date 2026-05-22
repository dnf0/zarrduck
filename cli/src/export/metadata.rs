use color_eyre::eyre::{eyre, Result as EyreResult};
use std::sync::Arc;

pub struct MetadataBuilder<'a> {
    pub output: &'a str,
    pub shape: Vec<u64>,
    pub data_type: zarrs::array::DataType,
    pub coord_columns: Vec<String>,
    pub chunks: Option<String>,
}

impl<'a> MetadataBuilder<'a> {
    pub async fn build_array(
        &self,
    ) -> EyreResult<(
        Arc<zarrs::array::Array<dyn zarrs::storage::AsyncWritableStorageTraits>>,
        Vec<u64>,
    )> {
        let store =
            geozarr_core::store::resolve_async_store(self.output).map_err(|e| eyre!("{}", e))?;

        let mut chunk_shape = Vec::new();
        let mut current_volume = 1u64;
        for &dim in &self.shape {
            let chunk_dim = if current_volume.saturating_mul(dim) <= 10_000_000 {
                dim
            } else {
                std::cmp::max(1, 10_000_000 / current_volume)
            };
            chunk_shape.push(chunk_dim);
            current_volume = current_volume.saturating_mul(chunk_dim);
        }

        if let Some(c_str) = &self.chunks {
            if let Ok(user_chunks) = serde_json::from_str::<serde_json::Value>(c_str) {
                if let Some(user_obj) = user_chunks.as_object() {
                    for (i, coord_name) in self.coord_columns.iter().enumerate() {
                        if let Some(val) = user_obj.get(coord_name) {
                            if let Some(dim_chunk) = val.as_u64() {
                                if i < chunk_shape.len() {
                                    chunk_shape[i] = dim_chunk;
                                }
                            }
                        }
                    }
                }
            } else {
                // Ignore parse errors, default auto-chunking will be used
            }
        }

        if chunk_shape.contains(&0) {
            return Err(eyre!("Chunk dimension size cannot be 0"));
        }

        let fill_value = match self.data_type {
            zarrs::array::DataType::Bool => zarrs::array::FillValue::from(false),
            zarrs::array::DataType::Int8 => zarrs::array::FillValue::from(0i8),
            zarrs::array::DataType::Int16 => zarrs::array::FillValue::from(0i16),
            zarrs::array::DataType::Int32 => zarrs::array::FillValue::from(0i32),
            zarrs::array::DataType::Int64 => zarrs::array::FillValue::from(0i64),
            zarrs::array::DataType::UInt8 => zarrs::array::FillValue::from(0u8),
            zarrs::array::DataType::UInt16 => zarrs::array::FillValue::from(0u16),
            zarrs::array::DataType::UInt32 => zarrs::array::FillValue::from(0u32),
            zarrs::array::DataType::UInt64 => zarrs::array::FillValue::from(0u64),
            zarrs::array::DataType::Float32 => zarrs::array::FillValue::from(f32::NAN),
            zarrs::array::DataType::Float64 => zarrs::array::FillValue::from(f64::NAN),
            zarrs::array::DataType::String => zarrs::array::FillValue::from(""),
            _ => return Err(eyre!("Unsupported DataType for FillValue")),
        };

        let array_builder = zarrs::array::ArrayBuilder::new(
            self.shape.clone(),
            self.data_type.clone(),
            chunk_shape.clone().try_into().unwrap(),
            fill_value,
        );

        let array = array_builder.build(store.clone(), "/").unwrap();
        array.async_store_metadata().await?;

        Ok((Arc::new(array), chunk_shape))
    }
}
