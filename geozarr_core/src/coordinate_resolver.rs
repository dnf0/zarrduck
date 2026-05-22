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
                if let Ok(parent_store) = crate::store::resolve_sync_store(parent) {
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
