import re

with open("extension/src/table_function.rs", "r") as f:
    content = f.read()

# Replace the DataType::String block
old_block = r"""                loop \{
                    if local_state\.current_chunk_buffer\.is_none\(\) \{
                        let mut g_state = init_data
                            \.global_state
                            \.lock\(\)
                            \.map_err\(\|e\| format\!\("Mutex poisoned: \{\}", e\)\)\?;
                        if g_state\.exhausted \{
                            break;
                        \}

                        local_state\.assigned_grid = g_state\.current_chunk_grid\.clone\(\);

                        let mut grid_shape = vec\!\[0u64; rank\];
                        let mut chunk_bounds_min = vec\!\[0u64; rank\];
                        let mut chunk_bounds_max = vec\!\[0u64; rank\];
                        for i in 0\.\.rank \{
                            grid_shape\[i\] = \(bind_data\.shape\[i\] as f64
                                / bind_data\.chunk_shape\[i\] as f64\)
                                \.ceil\(\) as u64;
                            chunk_bounds_min\[i\] =
                                bind_data\.bounds_min\[i\] / bind_data\.chunk_shape\[i\];
                            chunk_bounds_max\[i\] =
                                bind_data\.bounds_max\[i\] / bind_data\.chunk_shape\[i\];
                        \}
                        if \!crate::table_function::increment_chunk_grid\(
                            &mut g_state\.current_chunk_grid,
                            &grid_shape,
                            &chunk_bounds_min,
                            &chunk_bounds_max,
                        \) \{
                            g_state\.exhausted = true;
                        \}
                        drop\(g_state\);

                        let mut subset_start = vec\!\[0u64; rank\];
                        let mut subset_shape = vec\!\[0u64; rank\];
                        let mut global_starts = vec\!\[0u64; rank\];
                        let mut strides = vec\!\[1u64; rank\];
                        for d in 0\.\.rank \{
                            let chunk_start =
                                local_state\.assigned_grid\[d\] \* bind_data\.chunk_shape\[d\];
                            let chunk_end_inc = chunk_start \+ bind_data\.chunk_shape\[d\] - 1;
                            let lo = bind_data\.bounds_min\[d\]\.max\(chunk_start\);
                            let hi = bind_data\.bounds_max\[d\]\.min\(chunk_end_inc\);
                            subset_start\[d\] = lo - chunk_start;
                            subset_shape\[d\] = hi - lo \+ 1;
                            global_starts\[d\] = lo;
                        \}
                        for d in \(0\.\.rank - 1\)\.rev\(\) \{
                            strides\[d\] = strides\[d \+ 1\] \* subset_shape\[d \+ 1\];
                        \}

                        let elements: Vec<String> = if bind_data\.is_remote \{
                            let full = bind_data
                                \.array
                                \.retrieve_chunk_elements::<String>\(&local_state\.assigned_grid\)
                                \.map_err\(\|e\| format\!\("zarrs read error: \{\}", e\)\)\?;
                            let actual_chunk_shape: Vec<u64> = \(0\.\.rank\)
                                \.map\(\|d\| \{
                                    let chunk_start =
                                        local_state\.assigned_grid\[d\] \* bind_data\.chunk_shape\[d\];
                                    let chunk_end_inc = chunk_start \+ bind_data\.chunk_shape\[d\] - 1;
                                    let actual_end = bind_data\.shape\[d\]\.min\(chunk_end_inc \+ 1\);
                                    actual_end - chunk_start
                                \}\)
                                \.collect\(\);
                            let mut chunk_strides = vec\!\[1u64; rank\];
                            for d in \(0\.\.rank - 1\)\.rev\(\) \{
                                chunk_strides\[d\] = chunk_strides\[d \+ 1\] \* actual_chunk_shape\[d \+ 1\];
                            \}
                            let total: u64 = subset_shape\.iter\(\)\.product\(\);
                            let mut out = Vec::with_capacity\(total as usize\);
                            let mut idx = subset_start\.clone\(\);
                            for _ in 0\.\.total \{
                                let flat: u64 = \(0\.\.rank\)\.map\(\|d\| idx\[d\] \* chunk_strides\[d\]\)\.sum\(\);
                                out\.push\(full\[flat as usize\]\.clone\(\)\);
                                for d in \(0\.\.rank\)\.rev\(\) \{
                                    idx\[d\] \+= 1;
                                    if idx\[d\] < subset_start\[d\] \+ subset_shape\[d\] \{
                                        break;
                                    \}
                                    idx\[d\] = subset_start\[d\];
                                \}
                            \}
                            out
                        \} else \{
                            let chunk_subset = ArraySubset::new_with_start_shape\(
                                subset_start,
                                subset_shape\.clone\(\),
                            \)
                            \.map_err\(\|e\| format\!\("Invalid chunk subset: \{\}", e\)\)\?;
                            bind_data
                                \.array
                                \.retrieve_chunk_subset_elements::<String>\(
                                    &local_state\.assigned_grid,
                                    &chunk_subset,
                                \)
                                \.map_err\(\|e\| format\!\("zarrs read error: \{\}", e\)\)\?
                        \};

                        if elements\.is_empty\(\) \{
                            continue;
                        \}

                        local_state\.current_chunk_buffer = Some\(ChunkBuffer::String\(elements\)\);
                        local_state\.subset_global_starts = global_starts;
                        local_state\.subset_shape = subset_shape;
                        local_state\.subset_strides = strides;
                        local_state\.element_cursor = 0;
                    \}"""

new_block = """                loop {
                    if local_state.current_chunk_buffer.is_none() {
                        let mut g_state = init_data
                            .global_state
                            .lock()
                            .map_err(|e| format!("Mutex poisoned: {}", e))?;
                        
                        let assigned_grid = g_state.grid_iterator.next();
                        drop(g_state);

                        let assigned_grid = match assigned_grid {
                            Some(grid) => grid,
                            None => break,
                        };
                        local_state.assigned_grid = assigned_grid.clone();

                        let chunk_reader = crate::engine::ChunkReader::new(
                            bind_data.array.clone(),
                            bind_data.is_remote,
                            bind_data.shape.clone(),
                            bind_data.chunk_shape.clone(),
                        );

                        let (elements, subset_info) = chunk_reader.read_chunk_subset::<String>(
                            &assigned_grid,
                            &bind_data.bounds_min,
                            &bind_data.bounds_max,
                        ).map_err(|e| format!("zarrs read error: {}", e))?;

                        if elements.is_empty() {
                            continue;
                        }

                        local_state.current_chunk_buffer = Some(ChunkBuffer::String(elements));
                        local_state.subset_global_starts = subset_info.global_starts;
                        local_state.subset_shape = subset_info.shape;
                        local_state.subset_strides = subset_info.strides;
                        local_state.element_cursor = 0;
                    }"""

content = re.sub(old_block, new_block, content, flags=re.MULTILINE)

with open("extension/src/table_function.rs", "w") as f:
    f.write(content)
