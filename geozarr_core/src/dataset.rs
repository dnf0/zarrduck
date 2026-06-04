use crate::metadata::SpatialTransform;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zarrs::array::{Array, ArrayMetadata, DataType};

pub struct GeoZarrDataset {
    pub array: Arc<Array<dyn zarrs::storage::ReadableStorageTraits>>,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    pub data_type: DataType,
    pub dim_names: Vec<String>,
    pub coords: HashMap<String, Vec<f64>>,
    pub lon_0_360_dims: HashSet<usize>,
    pub spatial_transform: Option<SpatialTransform>,
    pub is_remote: bool,
    pub fill_value_bytes: Option<Vec<u8>>,
}

impl GeoZarrDataset {
    pub fn open(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let resolved_store = crate::store::resolve_sync_store(path)?;
        let is_remote = resolved_store.is_remote;
        let store_arc = resolved_store.store;

        let array = Array::open(Arc::clone(&store_arc), "/").map_err(
            |e| -> Box<dyn std::error::Error> { format!("zarrs error (array): {}", e).into() },
        )?;

        let shape = array.shape().to_vec();
        let rank = shape.len();
        if rank > 16 {
            return Err(format!(
                "Zarr array rank {} exceeds maximum supported dimensions (16)",
                rank
            )
            .into());
        }

        let metadata = array.metadata();

        let mut spatial_transform = None;
        if let ArrayMetadata::V2(meta) = metadata {
            if let Some(geozarr_meta) =
                crate::metadata::parse_geozarr_metadata(&Value::Object(meta.attributes.clone()))
            {
                spatial_transform = geozarr_meta.transform;
            }
        } else if let ArrayMetadata::V3(meta) = metadata {
            if let Some(geozarr_meta) =
                crate::metadata::parse_geozarr_metadata(&Value::Object(meta.attributes.clone()))
            {
                spatial_transform = geozarr_meta.transform;
            }
        }

        let dim_names = Self::resolve_dimension_names(metadata, rank);

        let (coords, lon_0_360_dims) = crate::coordinate_resolver::CoordinateResolver::resolve(
            path,
            Arc::clone(&store_arc),
            &shape,
            &dim_names,
        )?;

        let chunk_shape: Vec<u64> = array
            .chunk_grid()
            .chunk_shape(&vec![0; rank], &shape)
            .map_err(|_| -> Box<dyn std::error::Error> {
                "zarrs error: array bounds are out of grid".into()
            })?
            .ok_or_else(|| -> Box<dyn std::error::Error> {
                "zarrs error: array has no chunk shape".into()
            })?
            .iter()
            .map(|n| n.get())
            .collect();

        let data_type = array.data_type().clone();

        crate::validator::DatasetValidator::validate_chunk_sizes(&shape, &chunk_shape, &data_type)
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

        let fv_bytes = array.fill_value().as_ne_bytes().to_vec();
        let fill_value_bytes = if fv_bytes.is_empty() {
            None
        } else {
            Some(fv_bytes)
        };

        Ok(Self {
            array: Arc::new(array),
            shape,
            chunk_shape,
            data_type,
            dim_names,
            coords,
            lon_0_360_dims,
            spatial_transform,
            is_remote,
            fill_value_bytes,
        })
    }

    pub fn schema(&self) -> Result<Vec<(String, zarrs::array::DataType)>, String> {
        let mut schema = Vec::new();
        for (i, name) in self.dim_names.iter().enumerate() {
            let has_transform = self
                .spatial_transform
                .as_ref()
                .is_some_and(|t| i < t.scale.len());
            if self.coords.contains_key(name) || has_transform {
                schema.push((name.clone(), zarrs::array::DataType::Float64));
            } else {
                schema.push((name.clone(), zarrs::array::DataType::Int64));
            }
        }
        schema.push(("value".to_string(), self.data_type.clone()));
        Ok(schema)
    }

    pub fn compute_bounds(
        &self,
        constraints: &crate::query_planner::QueryConstraints,
    ) -> (Vec<u64>, Vec<u64>) {
        let rank = self.shape.len();
        let mut bounds_min = vec![0; rank];
        let mut bounds_max = vec![0; rank];
        for (i, max_val) in bounds_max.iter_mut().enumerate().take(rank) {
            *max_val = if self.shape[i] > 0 {
                self.shape[i] - 1
            } else {
                0
            };
        }

        for (dim_index, name) in self.dim_names.iter().enumerate() {
            if let Some(&pinned_idx) = constraints.pins.get(name) {
                let target = std::cmp::min(pinned_idx, bounds_max[dim_index]);
                bounds_min[dim_index] = target;
                bounds_max[dim_index] = target;
                continue;
            }

            let (min_val_opt, max_val_opt) = constraints
                .bounds
                .get(name)
                .copied()
                .unwrap_or((None, None));

            if let Some(coord_vals) = self.coords.get(name) {
                let normalize_query = |v: f64| -> f64 {
                    crate::coordinates::denormalize_longitude(
                        v,
                        self.lon_0_360_dims.contains(&dim_index),
                    )
                };
                let is_ascending = coord_vals
                    .first()
                    .zip(coord_vals.last())
                    .is_none_or(|(f, l)| f <= l);
                if let Some(min_val) = min_val_opt {
                    let (t_min, t_max) = crate::query_planner::translate_filter(
                        coord_vals,
                        ">=",
                        normalize_query(min_val),
                        bounds_min[dim_index],
                        bounds_max[dim_index],
                    );
                    if is_ascending {
                        bounds_min[dim_index] = std::cmp::max(bounds_min[dim_index], t_min);
                    } else {
                        bounds_max[dim_index] = std::cmp::min(bounds_max[dim_index], t_max);
                    }
                }
                if let Some(max_val) = max_val_opt {
                    let (t_min, t_max) = crate::query_planner::translate_filter(
                        coord_vals,
                        "<=",
                        normalize_query(max_val),
                        bounds_min[dim_index],
                        bounds_max[dim_index],
                    );
                    if is_ascending {
                        bounds_max[dim_index] = std::cmp::min(bounds_max[dim_index], t_max);
                    } else {
                        bounds_min[dim_index] = std::cmp::max(bounds_min[dim_index], t_min);
                    }
                }
            } else if let Some(ref transform) = self.spatial_transform {
                if dim_index < transform.scale.len() {
                    let scale = transform.scale[dim_index];
                    let translation = transform.translation.get(dim_index).copied().unwrap_or(0.0);

                    if scale != 0.0 {
                        if let Some(min_val) = min_val_opt {
                            let idx1 = ((min_val - translation) / scale).floor() as i64;
                            let idx2 = ((min_val - translation) / scale).ceil() as i64;
                            let mut target_min = if scale > 0.0 { idx1 } else { idx2 };
                            if target_min < 0 {
                                target_min = 0;
                            }

                            if scale > 0.0 {
                                bounds_min[dim_index] =
                                    std::cmp::max(bounds_min[dim_index], target_min as u64);
                            } else {
                                bounds_max[dim_index] =
                                    std::cmp::min(bounds_max[dim_index], target_min as u64);
                            }
                        }

                        if let Some(max_val) = max_val_opt {
                            let idx1 = ((max_val - translation) / scale).floor() as i64;
                            let idx2 = ((max_val - translation) / scale).ceil() as i64;
                            let mut target_max = if scale > 0.0 { idx2 } else { idx1 };
                            if target_max < 0 {
                                target_max = 0;
                            }

                            if scale > 0.0 {
                                bounds_max[dim_index] =
                                    std::cmp::min(bounds_max[dim_index], target_max as u64);
                            } else {
                                bounds_min[dim_index] =
                                    std::cmp::max(bounds_min[dim_index], target_max as u64);
                            }
                        }
                    }
                }
            }
        }

        (bounds_min, bounds_max)
    }

    fn resolve_dimension_names(metadata: &ArrayMetadata, rank: usize) -> Vec<String> {
        let attributes = match metadata {
            ArrayMetadata::V2(meta) => &meta.attributes,
            ArrayMetadata::V3(meta) => &meta.attributes,
        };

        if let Some(Value::Array(dims)) = attributes.get("_ARRAY_DIMENSIONS") {
            if dims.len() == rank {
                let names: Option<Vec<String>> = dims
                    .iter()
                    .map(|dim| {
                        if let Value::String(s) = dim {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if let Some(names) = names {
                    return names;
                }
            }
        }

        (0..rank).map(|i| format!("dim_{}", i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_dimension_names_fallback() {
        let json_meta = r#"{
            "zarr_format": 2,
            "shape": [1, 2, 3],
            "chunks": [1, 2, 3],
            "dtype": "<i4",
            "compressor": null,
            "fill_value": null,
            "filters": null,
            "order": "C"
        }"#;
        let metadata_bare: ArrayMetadata = serde_json::from_str(json_meta).unwrap();
        let names = GeoZarrDataset::resolve_dimension_names(&metadata_bare, 3);
        assert_eq!(names, vec!["dim_0", "dim_1", "dim_2"]);
    }

    #[test]
    fn test_resolve_dimension_names_with_attributes() {
        let json_meta = r#"{
            "zarr_format": 2,
            "shape": [1, 2, 3],
            "chunks": [1, 2, 3],
            "dtype": "<i4",
            "compressor": null,
            "fill_value": null,
            "filters": null,
            "order": "C",
            "attributes": {
                "_ARRAY_DIMENSIONS": ["time", "lat", "lon"]
            }
        }"#;
        let metadata_attrs: ArrayMetadata = serde_json::from_str(json_meta).unwrap();
        let names = GeoZarrDataset::resolve_dimension_names(&metadata_attrs, 3);
        assert_eq!(names, vec!["time", "lat", "lon"]);
    }
}
