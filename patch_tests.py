import re

with open('extension/tests/test_extension.rs', 'r') as f:
    content = f.read()

# We want to replace #[test]\nfn test_read_zarr_schema() -> Result<()> { ... }
# with the new split functions.

start_str = "#[test]\nfn test_read_zarr_schema() -> Result<()> {"
end_str = "#[test]\nfn test_geozarr_spatial_metadata() -> duckdb::Result<()> {"

start_idx = content.find(start_str)
end_idx = content.find(end_str)

if start_idx == -1 or end_idx == -1:
    print("Could not find start or end index.")
    exit(1)

helper_code = """
fn setup_mock_zarr() -> Result<(duckdb::Connection, tempfile::TempDir, std::path::PathBuf)> {
    let conn = duckdb::Connection::open_in_memory()?;
    conn.register_table_function::<zarrduck::ReadZarrVTab>("read_zarr")?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());
    let mut builder = ArrayBuilder::new(
        vec![10, 20],
        DataType::Float32,
        vec![5, 5].try_into().unwrap(),
        FillValue::from(0.0f32),
    );

    let mut attributes = serde_json::Map::new();
    attributes.insert(
        "_ARRAY_DIMENSIONS".to_string(),
        serde_json::json!(["time", "lat"]),
    );
    builder.attributes(attributes);

    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();

    let lat_builder = ArrayBuilder::new(
        vec![20],
        DataType::Float32,
        vec![20].try_into().unwrap(),
        FillValue::from(0.0f32),
    );
    let lat_array = lat_builder.build(Arc::clone(&store), "/lat").unwrap();
    lat_array.store_metadata().unwrap();
    let lat_data: Vec<f32> = (0..20).map(|i| 45.0 + i as f32).collect();
    lat_array.store_chunk_elements(&[0], &lat_data).unwrap();

    let mut val = 0.0f32;
    for t_c in 0..2 {
        for l_c in 0..4 {
            let chunk_data: Vec<f32> = (0..25)
                .map(|_| {
                    let v = val;
                    val += 1.0;
                    v
                })
                .collect();
            array
                .store_chunk_elements(&[t_c, l_c], &chunk_data)
                .unwrap();
        }
    }

    Ok((conn, temp_dir, store_path))
}

#[test]
fn test_schema_basic_types() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let query_info = format!(
        "DESCRIBE SELECT * FROM read_zarr('{}')",
        store_path.display()
    );
    let mut info_stmt = conn.prepare(&query_info)?;
    let mut rows = info_stmt.query([])?;

    let mut column_names = Vec::new();
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        column_names.push(col_name);
    }

    assert_eq!(column_names, vec!["time", "lat", "value"]);

    let mut info_stmt = conn.prepare(&query_info)?;
    let mut rows = info_stmt.query([])?;

    let mut column_types = Vec::new();
    while let Some(row) = rows.next()? {
        let col_type: String = row.get(1)?;
        column_types.push(col_type);
    }
    assert_eq!(column_types, vec!["BIGINT", "DOUBLE", "FLOAT"]);

    let max_lat: f64 = conn.query_row(
        &format!("SELECT max(lat) FROM read_zarr('{}')", store_path.display()),
        [],
        |row| row.get(0),
    )?;
    assert_eq!(max_lat, 64.0);

    let count: i64 = conn.query_row(
        &format!("SELECT count(*) FROM read_zarr('{}')", store_path.display()),
        [],
        |row| row.get(0),
    )?;
    assert_eq!(count, 200);

    Ok(())
}

#[test]
fn test_schema_named_parameters() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let query_params = format!(
        "SELECT count(*) FROM read_zarr('{}', lat_min := 50.0, lat_max := 55.0)",
        store_path.display()
    );
    let mut stmt_params = conn.prepare(&query_params)?;
    let count_params: i64 = stmt_params.query_row([], |row| row.get(0))?;
    assert_eq!(count_params, 60);

    Ok(())
}

#[test]
fn test_schema_projection_pushdown_value() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let query_proj = format!(
        "SELECT SUM(value) FROM read_zarr('{}')",
        store_path.display()
    );
    let mut stmt_proj = conn.prepare(&query_proj)?;
    let sum_val: f64 = stmt_proj.query_row([], |row| row.get(0))?;
    assert_eq!(sum_val, 19900.0);

    Ok(())
}

#[test]
fn test_schema_null_mapping() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let query_null = format!(
        "SELECT count(value) FROM read_zarr('{}')",
        store_path.display()
    );
    let mut stmt_null = conn.prepare(&query_null)?;
    let non_null_count: i64 = stmt_null.query_row([], |row| row.get(0))?;
    assert_eq!(non_null_count, 199);

    Ok(())
}

#[test]
fn test_schema_projection_pushdown_coord() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let query_coord_proj = format!("SELECT SUM(lat) FROM read_zarr('{}')", store_path.display());
    let mut stmt_coord_proj = conn.prepare(&query_coord_proj)?;
    let sum_lat: f64 = stmt_coord_proj.query_row([], |row| row.get(0))?;
    assert_eq!(sum_lat, 10900.0);

    Ok(())
}

#[test]
fn test_schema_corrupted_chunk() -> Result<()> {
    let (conn, _temp_dir, store_path) = setup_mock_zarr()?;

    let chunk_path = store_path.join("c").join("0").join("0");
    std::fs::write(&chunk_path, vec![0u8; 1]).unwrap();
    let query_corrupt = format!(
        "SELECT SUM(value) FROM read_zarr('{}')",
        store_path.display()
    );
    let mut stmt_corrupt = conn.prepare(&query_corrupt)?;
    let result = stmt_corrupt.query_row([], |row| row.get::<_, f64>(0));
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("zarrs read error"));

    Ok(())
}
"""

new_content = content[:start_idx] + helper_code + "\n" + content[end_idx:]

with open('extension/tests/test_extension.rs', 'w') as f:
    f.write(new_content)

print("Patch applied successfully.")
