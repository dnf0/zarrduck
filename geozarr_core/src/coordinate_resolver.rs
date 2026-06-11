use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zarrs::array::Array;

pub struct CoordinateResolver;

impl CoordinateResolver {
    #[allow(clippy::type_complexity)]
    pub fn resolve(
        path: &str,
        store_arc: Arc<dyn zarrs::storage::ReadableStorageTraits>,
        shape: &[u64],
        dim_names: &[String],
    ) -> Result<(HashMap<String, Vec<f64>>, HashSet<usize>), Box<dyn std::error::Error>> {
        let mut coords = HashMap::new();
        let mut lon_0_360_dims = HashSet::new();

        for (dim_index, name) in dim_names.iter().enumerate() {
            if let Ok(coord_array) = Array::open(Arc::clone(&store_arc), &format!("/{}", name)) {
                if coord_array.shape().len() == 1 && coord_array.shape()[0] < 1_000_000 {
                    let subset = zarrs::array_subset::ArraySubset::new_with_shape(
                        coord_array.shape().to_vec(),
                    );
                    let vals_result: Result<Vec<f64>, _> = match coord_array.data_type() {
                        zarrs::array::DataType::Float64 => {
                            coord_array.retrieve_array_subset_elements::<f64>(&subset)
                        }
                        zarrs::array::DataType::Float32 => coord_array
                            .retrieve_array_subset_elements::<f32>(&subset)
                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                        zarrs::array::DataType::Int64 => coord_array
                            .retrieve_array_subset_elements::<i64>(&subset)
                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                        zarrs::array::DataType::Int32 => coord_array
                            .retrieve_array_subset_elements::<i32>(&subset)
                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                        _ => continue,
                    };

                    if let Ok(vals) = vals_result {
                        if vals.len() as u64 == shape[dim_index] {
                            let is_lon = name == "lon" || name == "longitude";
                            if is_lon && vals.iter().any(|&v| v > 180.0) {
                                lon_0_360_dims.insert(dim_index);
                            }
                            coords.insert(name.clone(), vals);
                        }
                    }
                }
            }
        }

        let missing_dims: Vec<usize> = dim_names
            .iter()
            .enumerate()
            .filter(|(_, name)| !coords.contains_key(*name))
            .map(|(i, _)| i)
            .collect();

        if !missing_dims.is_empty() {
            let parent_path = path
                .trim_end_matches('/')
                .rsplit_once('/')
                .map(|(p, _)| p.to_string());
            if let Some(ref parent) = parent_path {
                if let Ok(parent_store) = crate::store::resolve_sync_store(parent, None) {
                    for dim_index in &missing_dims {
                        let name = &dim_names[*dim_index];
                        if let Ok(coord_array) =
                            Array::open(Arc::clone(&parent_store.store), &format!("/{}", name))
                        {
                            if coord_array.shape().len() == 1
                                && coord_array.shape()[0] < 1_000_000
                                && coord_array.shape()[0] == shape[*dim_index]
                            {
                                let subset = zarrs::array_subset::ArraySubset::new_with_shape(
                                    coord_array.shape().to_vec(),
                                );
                                let vals_result: Result<Vec<f64>, _> =
                                    match coord_array.data_type() {
                                        zarrs::array::DataType::Float64 => coord_array
                                            .retrieve_array_subset_elements::<f64>(&subset),
                                        zarrs::array::DataType::Float32 => coord_array
                                            .retrieve_array_subset_elements::<f32>(&subset)
                                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                                        zarrs::array::DataType::Int64 => coord_array
                                            .retrieve_array_subset_elements::<i64>(&subset)
                                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                                        zarrs::array::DataType::Int32 => coord_array
                                            .retrieve_array_subset_elements::<i32>(&subset)
                                            .map(|v| v.into_iter().map(|x| x as f64).collect()),
                                        _ => continue,
                                    };
                                if let Ok(vals) = vals_result {
                                    let is_lon = name == "lon" || name == "longitude";
                                    if is_lon && vals.iter().any(|&v| v > 180.0) {
                                        lon_0_360_dims.insert(*dim_index);
                                    }
                                    coords.insert(name.clone(), vals);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((coords, lon_0_360_dims))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use zarrs::array::codec::BytesCodec;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::array_subset::ArraySubset;
    use zarrs::storage::store::MemoryStore;

    #[test]
    fn test_coordinate_resolver_types_and_lon() {
        let store = Arc::new(MemoryStore::new());
        let chunk_shape = vec![NonZeroU64::new(2).unwrap()];
        let time_chunk_shape = vec![NonZeroU64::new(1).unwrap()];

        // Create an f32 lat array
        let lat_array = ArrayBuilder::new(
            vec![2],
            DataType::Float32,
            chunk_shape.clone().into(),
            FillValue::from(0.0f32),
        )
        .array_to_bytes_codec(Box::new(BytesCodec::default()))
        .build(store.clone(), "/lat")
        .unwrap();
        lat_array.store_metadata().unwrap();
        lat_array
            .store_array_subset_elements::<f32>(
                &ArraySubset::new_with_shape(vec![2]),
                &[45.0, 46.0],
            )
            .unwrap();

        // Create an f64 lon array with 0-360 values
        let lon_array = ArrayBuilder::new(
            vec![2],
            DataType::Float64,
            chunk_shape.clone().into(),
            FillValue::from(0.0f64),
        )
        .array_to_bytes_codec(Box::new(BytesCodec::default()))
        .build(store.clone(), "/lon")
        .unwrap();
        lon_array.store_metadata().unwrap();
        lon_array
            .store_array_subset_elements::<f64>(
                &ArraySubset::new_with_shape(vec![2]),
                &[179.0, 181.0],
            )
            .unwrap();

        // Create an i32 time array
        let time_array = ArrayBuilder::new(
            vec![1],
            DataType::Int32,
            time_chunk_shape.into(),
            FillValue::from(0i32),
        )
        .array_to_bytes_codec(Box::new(BytesCodec::default()))
        .build(store.clone(), "/time")
        .unwrap();
        time_array.store_metadata().unwrap();
        time_array
            .store_array_subset_elements::<i32>(&ArraySubset::new_with_shape(vec![1]), &[2020])
            .unwrap();

        // Resolve
        let dim_names = vec!["time".to_string(), "lat".to_string(), "lon".to_string()];
        let shape = vec![1, 2, 2];

        let (coords, lon_0_360) =
            CoordinateResolver::resolve("mock://path", store, &shape, &dim_names).unwrap();

        // Verify time (i32 -> f64)
        assert_eq!(coords.get("time").unwrap(), &[2020.0]);

        // Verify lat (f32 -> f64)
        assert_eq!(coords.get("lat").unwrap(), &[45.0, 46.0]);

        // Verify lon (0-360 tracking)
        assert_eq!(coords.get("lon").unwrap(), &[179.0, 181.0]);
        assert!(lon_0_360.contains(&2)); // The 3rd dim (index 2) is 0-360
    }
}
