use zarrs::array::DataType;

pub struct DatasetValidator;

impl DatasetValidator {
    pub fn validate_chunk_sizes(
        _shape: &[u64],
        chunk_shape: &[u64],
        data_type: &DataType,
    ) -> Result<(), String> {
        if chunk_shape.contains(&0) {
            return Err("Chunk dimension size cannot be 0".to_string());
        }

        let chunk_volume = chunk_shape
            .iter()
            .try_fold(1u64, |acc, &x| acc.checked_mul(x))
            .ok_or_else(|| "Chunk volume overflow".to_string())?;

        if *data_type == DataType::String {
            if chunk_volume > 1_000_000 {
                return Err(format!(
                    "Zarr string chunk volume {} exceeds maximum allowed (1,000,000 elements) to prevent OOM",
                    chunk_volume
                ));
            }
        } else {
            let bytes_per_element = match data_type {
                DataType::Float64 | DataType::Int64 | DataType::UInt64 => 8,
                DataType::Float32 | DataType::Int32 | DataType::UInt32 => 4,
                DataType::Int16 | DataType::UInt16 => 2,
                _ => 1,
            };
            let chunk_bytes = chunk_volume
                .checked_mul(bytes_per_element)
                .ok_or_else(|| "Chunk byte volume overflow".to_string())?;
            if chunk_bytes > 256 * 1024 * 1024 {
                return Err(format!(
                    "Chunk size {} bytes exceeds maximum allowed volume of 256MB",
                    chunk_bytes
                ));
            }
        }

        Ok(())
    }
}
