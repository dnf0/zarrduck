with open('extension/src/table_function.rs', 'r') as f:
    content = f.read()

idx = content.find('    #[test]\n    fn test_resolve_dimension_names_with_attributes() {')
if idx != -1:
    new_content = content[:idx] + """#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iteration_state_initialization() {
        let _global_state = GlobalState {
            grid_iterator: crate::engine::GridIterator::new(
                &[0, 0, 0],
                &[10, 10, 10],
                &[10, 10, 10],
                &[5, 5, 5],
            ),
        };
        let local_state = LocalState {
            assigned_grid: vec![0, 0, 0],
            element_cursor: 0,
            current_chunk_buffer: None,
            projected_columns: vec![0, 1, 2],
            subset_global_starts: vec![],
            subset_shape: vec![],
            subset_strides: vec![],
        };
        assert_eq!(local_state.element_cursor, 0);
    }
}
"""
    with open('extension/src/table_function.rs', 'w') as f:
        f.write(new_content)
