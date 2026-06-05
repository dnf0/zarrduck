use duckdb::{Connection, Result};
use std::path::Path;

#[test]
fn test_new_data_types() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;

    // We don't set GEOZARR_ALLOW_PATH dynamically to avoid race conditions in parallel tests.
    // Instead, the tests will be run with GEOZARR_ALLOW_PATH set for the whole test process
    // via a global setup, or we just rely on the component scanner allowing /tmp.
    // Wait, let's just create the temp directory inside the project target directory!
    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_types.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());

    // Test Boolean
    let bool_builder = ArrayBuilder::new(
        vec![5],
        DataType::Bool,
        vec![5].try_into().unwrap(),
        FillValue::from(false),
    );
    let bool_array = bool_builder.build(Arc::clone(&store), "/bool").unwrap();
    bool_array.store_metadata().unwrap();
    let bool_data: Vec<bool> = vec![true, false, true, true, false];
    bool_array.store_chunk_elements(&[0], &bool_data).unwrap();

    let query_bool = format!("SELECT * FROM read_geo('{}/bool')", store_path.display());
    let mut stmt_bool = conn.prepare(&query_bool)?;
    let mut rows_bool = stmt_bool.query([])?;

    let mut bool_results = Vec::new();
    while let Some(row) = rows_bool.next()? {
        bool_results.push(row.get::<_, Option<bool>>(1)?);
    }
    assert_eq!(
        bool_results,
        vec![Some(true), None, Some(true), Some(true), None]
    );

    // Test Int8
    let i8_builder = ArrayBuilder::new(
        vec![5],
        DataType::Int8,
        vec![5].try_into().unwrap(),
        FillValue::from(0i8),
    );
    let i8_array = i8_builder.build(Arc::clone(&store), "/i8").unwrap();
    i8_array.store_metadata().unwrap();
    let i8_data: Vec<i8> = vec![-10, 20, -30, 40, -50];
    i8_array.store_chunk_elements(&[0], &i8_data).unwrap();

    let query_i8 = format!("SELECT * FROM read_geo('{}/i8')", store_path.display());
    let mut stmt_i8 = conn.prepare(&query_i8)?;
    let mut rows_i8 = stmt_i8.query([])?;

    let mut i8_results = Vec::new();
    while let Some(row) = rows_i8.next()? {
        i8_results.push(row.get::<_, i8>(1)?);
    }
    assert_eq!(i8_results, vec![-10, 20, -30, 40, -50]);

    Ok(())
}

#[test]
fn test_read_geo_function_compiles() {
    let candidate_paths = vec![
        "../target/debug/eider_extension.duckdb_extension",
        "target/debug/eider_extension.duckdb_extension",
    ];

    // We just verify the extension file was successfully packaged by `cargo duckdb-ext build`.
    // Loading it dynamically into the host duckdb CLI is not tested here because
    // DuckDB extensions are strictly tied to the exact minor version (v1.1.x vs v1.5.x)
    // and will panic on load if there's a mismatch.
    let mut exists = candidate_paths.iter().any(|p| Path::new(p).exists());

    if !exists {
        // Try building it automatically for the test
        let _ = std::process::Command::new("cargo")
            .args(&["duckdb-ext", "build"])
            .status();

        exists = candidate_paths.iter().any(|p| Path::new(p).exists());
    }

    assert!(
        exists,
        "Extension file not found. Please run `cargo duckdb-ext build` before `cargo test`"
    );
}

fn setup_mock_zarr() -> Result<(duckdb::Connection, tempfile::TempDir, std::path::PathBuf)> {
    let conn = duckdb::Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;

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
        "DESCRIBE SELECT * FROM read_geo('{}')",
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
        &format!("SELECT max(lat) FROM read_geo('{}')", store_path.display()),
        [],
        |row| row.get(0),
    )?;
    assert_eq!(max_lat, 64.0);

    let count: i64 = conn.query_row(
        &format!("SELECT count(*) FROM read_geo('{}')", store_path.display()),
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
        "SELECT count(*) FROM read_geo('{}', lat_min := 50.0, lat_max := 55.0)",
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
        "SELECT SUM(value) FROM read_geo('{}')",
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
        "SELECT count(value) FROM read_geo('{}')",
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

    let query_coord_proj = format!("SELECT SUM(lat) FROM read_geo('{}')", store_path.display());
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
        "SELECT SUM(value) FROM read_geo('{}')",
        store_path.display()
    );
    let mut stmt_corrupt = conn.prepare(&query_corrupt)?;
    let result = stmt_corrupt.query_row([], |row| row.get::<_, f64>(0));
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("zarrs read error"));

    Ok(())
}

#[test]
fn test_geozarr_spatial_metadata() -> duckdb::Result<()> {
    let conn = duckdb::Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;
    conn.register_table_function::<eider::metadata_vtab::ReadZarrMetadataVTab>(
        "read_zarr_metadata",
    )?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_spatial.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());
    let mut builder = ArrayBuilder::new(
        vec![2, 2],
        DataType::Float32,
        vec![2, 2].try_into().unwrap(),
        FillValue::from(0.0f32),
    );

    let mut attributes = serde_json::Map::new();
    attributes.insert(
        "_ARRAY_DIMENSIONS".to_string(),
        serde_json::json!(["y", "x"]),
    );
    attributes.insert(
        "geozarr".to_string(),
        serde_json::json!({
            "crs": "EPSG:3857",
            "spatial_transform": {
                "scale": [-10.0, 10.0],
                "translation": [90.0, -180.0]
            }
        }),
    );
    builder.attributes(attributes);

    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();
    array
        .store_chunk_elements(&[0, 0], &[1.0f32, 2.0, 3.0, 4.0])
        .unwrap();

    // 1. Test Metadata
    let query_meta = format!(
        "SELECT crs FROM read_zarr_metadata('{}')",
        store_path.display()
    );
    let mut stmt_meta = conn.prepare(&query_meta)?;
    let crs: String = stmt_meta.query_row([], |row| row.get(0))?;
    assert_eq!(crs, "EPSG:3857");

    // 2. Test Spatial Coordinates Projection
    // y_idx=0, x_idx=0 -> y: 90 + (0 * -10) = 90.0 | x: -180 + (0 * 10) = -180.0
    // y_idx=0, x_idx=1 -> y: 90 + (0 * -10) = 90.0 | x: -180 + (1 * 10) = -170.0
    // y_idx=1, x_idx=0 -> y: 90 + (1 * -10) = 80.0 | x: -180 + (0 * 10) = -180.0

    let query_data = format!(
        "SELECT y, x, value FROM read_geo('{}') ORDER BY y DESC, x ASC",
        store_path.display()
    );
    let mut stmt_data = conn.prepare(&query_data)?;
    let mut rows = stmt_data.query([])?;

    let row1 = rows.next()?.unwrap();
    assert_eq!(row1.get::<_, f64>(0)?, 90.0); // y
    assert_eq!(row1.get::<_, f64>(1)?, -180.0); // x
    assert_eq!(row1.get::<_, f32>(2)?, 1.0); // value

    let row2 = rows.next()?.unwrap();
    assert_eq!(row2.get::<_, f64>(0)?, 90.0); // y
    assert_eq!(row2.get::<_, f64>(1)?, -170.0); // x
    assert_eq!(row2.get::<_, f32>(2)?, 2.0); // value

    Ok(())
}
