import re

with open("extension/src/table_function.rs", "r") as f:
    content = f.read()

# 1. Remove enum ChunkBuffer
content = re.sub(
    r"pub enum ChunkBuffer \{.*?\n\}\n\n",
    "use geozarr_core::types::ChunkBuffer;\n\n",
    content,
    flags=re.DOTALL
)

# 2. Update dispatch_yield_loop!
string_logic = """    (String, $enum_variant:path, $output:expr, $local_state:expr, $global_state:expr, $bind_data:expr) => {{
        let rank = $bind_data.shape.len();
        let mut value_vector = $output.flat_vector(rank);
        let mut valid_rows = 0;

        loop {
            if $local_state.current_chunk_buffer.is_none() {
                let mut g_state = $global_state
                    .lock()
                    .map_err(|e| format!("Mutex poisoned: {}", e))?;

                let assigned_grid = g_state.grid_iterator.next();
                drop(g_state);

                let assigned_grid = match assigned_grid {
                    Some(grid) => grid,
                    None => break,
                };
                $local_state.assigned_grid = assigned_grid.clone();

                let chunk_reader = crate::engine::ChunkReader::new(
                    $bind_data.array.clone(),
                    $bind_data.is_remote,
                    $bind_data.shape.clone(),
                    $bind_data.chunk_shape.clone(),
                );

                let (elements, subset_info) = chunk_reader
                    .read_chunk_subset::<String>(
                        &assigned_grid,
                        &$bind_data.bounds_min,
                        &$bind_data.bounds_max,
                    )
                    .map_err(|e| format!("zarrs read error: {}", e))?;

                if elements.is_empty() {
                    continue;
                }

                $local_state.current_chunk_buffer = Some($enum_variant(elements));
                $local_state.subset_global_starts = subset_info.global_starts;
                $local_state.subset_shape = subset_info.shape;
                $local_state.subset_strides = subset_info.strides;
                $local_state.element_cursor = 0;
            }

            let buffer = match $local_state.current_chunk_buffer.as_ref().unwrap() {
                $enum_variant(buf) => buf,
                _ => return Err("Chunk buffer type mismatch".into()),
            };

            let total = buffer.len();
            let batch_size = (total - $local_state.element_cursor).min(2048 - valid_rows);

            for dim in 0..rank {
                if $local_state.projected_columns.contains(&dim) {
                    let stride = $local_state.subset_strides[dim];
                    let dim_size = $local_state.subset_shape[dim];
                    let g_start = $local_state.subset_global_starts[dim];

                    if let Some(coord_vals) =
                        $bind_data.coords.get(&$bind_data.dim_names[dim])
                    {
                        let is_0_360 = $bind_data.lon_0_360_dims.contains(&dim);
                        let mut coord_vector = $output.flat_vector(dim);
                        let coord_slice = coord_vector.as_mut_slice::<f64>();
                        for i in 0..batch_size {
                            let pos = ($local_state.element_cursor + i) as u64;
                            let g_idx = (g_start + (pos / stride) % dim_size) as usize;
                            let raw = coord_vals.get(g_idx).copied().unwrap_or(f64::NAN);
                            coord_slice[valid_rows + i] =
                                geozarr_core::coordinates::normalize_longitude(
                                    raw, is_0_360,
                                );
                        }
                    } else if let Some(ref transform) = $bind_data.spatial_transform {
                        if dim < transform.scale.len() {
                            let mut coord_vector = $output.flat_vector(dim);
                            let coord_slice = coord_vector.as_mut_slice::<f64>();
                            for i in 0..batch_size {
                                let pos = ($local_state.element_cursor + i) as u64;
                                let g_idx = g_start + (pos / stride) % dim_size;
                                coord_slice[valid_rows + i] =
                                    geozarr_core::coordinates::apply_transform(
                                        transform, dim, g_idx,
                                    );
                            }
                        } else {
                            let mut coord_vector = $output.flat_vector(dim);
                            let coord_slice = coord_vector.as_mut_slice::<i64>();
                            for i in 0..batch_size {
                                let pos = ($local_state.element_cursor + i) as u64;
                                coord_slice[valid_rows + i] =
                                    (g_start + (pos / stride) % dim_size) as i64;
                            }
                        }
                    } else {
                        let mut coord_vector = $output.flat_vector(dim);
                        let coord_slice = coord_vector.as_mut_slice::<i64>();
                        for i in 0..batch_size {
                            let pos = ($local_state.element_cursor + i) as u64;
                            coord_slice[valid_rows + i] =
                                (g_start + (pos / stride) % dim_size) as i64;
                        }
                    }
                }
            }

            if $local_state.projected_columns.contains(&rank) {
                for i in 0..batch_size {
                    let val = buffer
                        .get($local_state.element_cursor + i)
                        .ok_or("Malformed Zarr chunk: unexpected buffer size")?;
                    if Some(val.as_bytes()) == $bind_data.fill_value_bytes.as_deref() {
                        value_vector.set_null(valid_rows + i);
                    } else {
                        value_vector.insert(valid_rows + i, val.as_str());
                    }
                }
            }

            valid_rows += batch_size;
            $local_state.element_cursor += batch_size;
            if $local_state.element_cursor >= total {
                $local_state.current_chunk_buffer = None;
            }

            if valid_rows >= 2048 {
                break;
            }
        }

        $output.set_len(valid_rows);
    }};
    ($rust_type:ty, $enum_variant:path, $output:expr, $local_state:expr, $global_state:expr, $bind_data:expr) => {{
"""

content = content.replace(
    "macro_rules! dispatch_yield_loop {\n    ($rust_type:ty, $enum_variant:path, $output:expr, $local_state:expr, $global_state:expr, $bind_data:expr) => {{\n",
    "macro_rules! dispatch_yield_loop {\n" + string_logic
)

# 3. Replace the match block
match_block_pattern = r"        // Dispatch based on data type\n        match bind_data\.data_type \{.*?\n            _ => return Err\(format!\(\"Unsupported data type: \{\:\?\}\", bind_data\.data_type\)\.into\(\)\),\n        \}"
replacement = """        // Dispatch based on data type
        geozarr_core::dispatch_zarr_type!(
            bind_data.data_type,
            dispatch_yield_loop,
            output,
            local_state,
            &init_data.global_state,
            bind_data
        );"""
content = re.sub(match_block_pattern, replacement, content, flags=re.DOTALL)

with open("extension/src/table_function.rs", "w") as f:
    f.write(content)
