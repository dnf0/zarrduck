import re

with open('extension/src/table_function.rs', 'r') as f:
    content = f.read()

pattern = re.compile(r'(fn bind\(bind: &BindInfo\) -> Result<Self::BindData, Box<dyn std::error::Error>> \{\n\s*if bind\.get_parameter_count\(\) < 1 \{.*?\n\s*\}\n)(?=\s*fn init\(_init: &InitInfo\))', re.DOTALL)

def replacement(m):
    return '''fn bind(bind: &BindInfo) -> Result<Self::BindData, Box<dyn std::error::Error>> {
        if bind.get_parameter_count() < 1 {
            return Err("read_zarr requires at least 1 parameter (path)".into());
        }

        let path = bind.get_parameter(0).to_string();
        let dataset = geozarr_core::dataset::GeoZarrDataset::open(&path)?;

        let rank = dataset.shape.len();
        
        for (i, name) in dataset.dim_names.iter().enumerate() {
            let has_transform = dataset.spatial_transform
                .as_ref()
                .is_some_and(|t| i < t.scale.len());
            if dataset.coords.contains_key(name) || has_transform {
                bind.add_result_column(name, LogicalTypeId::Double.into());
            } else {
                bind.add_result_column(name, LogicalTypeId::Bigint.into());
            }
        }

        let value_type = geozarr_core::types::zarr_to_duckdb_logical_type(&dataset.data_type)
            .map_err(Box::<dyn std::error::Error>::from)?;
        bind.add_result_column("value", value_type.into());

        let mut bounds_min = vec![0; rank];
        let mut bounds_max = vec![0; rank];
        for i in 0..rank {
            bounds_max[i] = if dataset.shape[i] > 0 { dataset.shape[i] - 1 } else { 0 };
        }

        for (dim_index, name) in dataset.dim_names.iter().enumerate() {
            let min_param_name = format!("{}_min", name);
            let max_param_name = format!("{}_max", name);

            let min_val_opt = bind
                .get_named_parameter(&min_param_name)
                .and_then(|v| v.to_string().parse::<f64>().ok());
            let max_val_opt = bind
                .get_named_parameter(&max_param_name)
                .and_then(|v| v.to_string().parse::<f64>().ok());

            if let Some(coord_vals) = dataset.coords.get(name) {
                let normalize_query = |v: f64| -> f64 {
                    geozarr_core::coordinates::denormalize_longitude(
                        v,
                        dataset.lon_0_360_dims.contains(&dim_index),
                    )
                };
                let is_ascending = coord_vals
                    .first()
                    .zip(coord_vals.last())
                    .is_none_or(|(f, l)| f <= l);
                if let Some(min_val) = min_val_opt {
                    let (t_min, t_max) = geozarr_core::query_planner::translate_filter(
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
                    let (t_min, t_max) = geozarr_core::query_planner::translate_filter(
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
            } else if let Some(ref transform) = dataset.spatial_transform {
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

        Ok(ReadZarrBindData {
            path,
            shape: dataset.shape,
            chunk_shape: dataset.chunk_shape,
            data_type: dataset.data_type,
            dim_names: dataset.dim_names,
            coords: dataset.coords,
            lon_0_360_dims: dataset.lon_0_360_dims,
            bounds_min,
            bounds_max,
            fill_value_bytes: dataset.fill_value_bytes,
            array: dataset.array,
            spatial_transform: dataset.spatial_transform,
            is_remote: dataset.is_remote,
        })
    }
'''

new_content, count = pattern.subn(replacement, content)
if count > 0:
    with open('extension/src/table_function.rs', 'w') as f:
        f.write(new_content)
    print("Replaced bind method.")
else:
    print("Could not find bind method.")
