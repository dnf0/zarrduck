use duckdb::{Connection, Result};

#[test]
fn test_plan_read_geo() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;
    conn.register_table_function::<eider::PlanReadGeoVTab>("plan_read_geo")?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_plan.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());

    // 2D Array: 100x100, chunks of 10x10. float32
    let builder = ArrayBuilder::new(
        vec![100, 100],
        DataType::Float32,
        vec![10, 10].try_into().unwrap(),
        FillValue::from(0.0f32),
    );
    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();

    let query = format!("SELECT * FROM plan_read_geo('{}')", store_path.display());
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let total_chunks: Option<i64> = row.get(3)?;
        let total_bytes: Option<i64> = row.get(4)?;
        assert_eq!(total_chunks, Some(100)); // 100 * 100 / (10 * 10) = 100 chunks
        assert_eq!(total_bytes, Some(40000)); // 100 chunks * 100 elements * 4 bytes
    } else {
        panic!("No rows returned");
    }

    Ok(())
}

#[test]
fn test_plan_read_geo_bounding_box_and_types() -> Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;
    conn.register_table_function::<eider::PlanReadGeoVTab>("plan_read_geo")?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_plan_bbox.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());

    // 2D Array: 50x50, chunks of 5x5. Int16 (2 bytes per element)
    let mut builder = ArrayBuilder::new(
        vec![50, 50],
        DataType::Int16,
        vec![5, 5].try_into().unwrap(),
        FillValue::from(0i16),
    );

    // Add spatial coordinates mapping to attributes
    let attributes = serde_json::json!({
        "_ARRAY_DIMENSIONS": ["lat", "lon"]
    });
    builder.attributes(attributes.as_object().unwrap().clone());

    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();

    // Create 1D coordinate arrays to allow bounding box filtering
    // lat: 0 to 49
    let lat_builder = ArrayBuilder::new(
        vec![50],
        DataType::Float64,
        vec![50].try_into().unwrap(),
        FillValue::from(0.0f64),
    );
    let lat_array = lat_builder.build(Arc::clone(&store), "/lat").unwrap();
    lat_array.store_metadata().unwrap();
    let lat_data: Vec<f64> = (0..50).map(|x| x as f64).collect();
    lat_array.store_chunk_elements(&[0], &lat_data).unwrap();

    // lon: 0 to 49
    let lon_builder = ArrayBuilder::new(
        vec![50],
        DataType::Float64,
        vec![50].try_into().unwrap(),
        FillValue::from(0.0f64),
    );
    let lon_array = lon_builder.build(Arc::clone(&store), "/lon").unwrap();
    lon_array.store_metadata().unwrap();
    let lon_data: Vec<f64> = (0..50).map(|x| x as f64).collect();
    lon_array.store_chunk_elements(&[0], &lon_data).unwrap();

    // Filter bounding box: lat_min=10, lat_max=24. lon_min=5, lon_max=9.
    // lat chunks: indices 10 to 24. Since chunks are 5x5, lat chunk indices: 2 to 4 (3 chunks)
    // lon chunks: indices 5 to 9. lon chunk indices: 1 to 1 (1 chunk)
    // total chunks = 3 * 1 = 3 chunks.
    // chunk volume = 25 elements. bytes per element = 2.
    // chunk bytes = 50 bytes.
    // total_bytes = 3 * 50 = 150 bytes.
    let query = format!(
        "SELECT * FROM plan_read_geo('{}', lat_min=10, lat_max=24, lon_min=5, lon_max=9)",
        store_path.display()
    );
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let total_chunks: Option<i64> = row.get(3)?;
        let total_bytes: Option<i64> = row.get(4)?;
        assert_eq!(total_chunks, Some(3));
        assert_eq!(total_bytes, Some(150));
    } else {
        panic!("No rows returned");
    }

    Ok(())
}

#[test]
fn test_plan_read_geo_v3_native_dimension_names_prunes() -> Result<()> {
    // End-to-end: a native Zarr v3 store that carries dimension names ONLY in the
    // native `dimension_names` field of zarr.json (NO `_ARRAY_DIMENSIONS` attr,
    // as a real xarray/zarr-python v3 store would). Pruning must engage: a small
    // sub-bbox must select strictly fewer chunks than the full extent.
    let conn = Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;
    conn.register_table_function::<eider::PlanReadGeoVTab>("plan_read_geo")?;

    let temp_dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let store_path = temp_dir.path().join("test_plan_v3.zarr");

    use std::sync::Arc;
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::FilesystemStore;

    let store = Arc::new(FilesystemStore::new(&store_path).unwrap());

    // 2D Array: 50x50, chunks of 5x5 => 100 chunks at full extent.
    // Dimension names supplied via the NATIVE v3 field, NOT via attributes.
    let mut builder = ArrayBuilder::new(
        vec![50, 50],
        DataType::Int16,
        vec![5, 5].try_into().unwrap(),
        FillValue::from(0i16),
    );
    builder.dimension_names(Some(["lat", "lon"]));
    let array = builder.build(Arc::clone(&store), "/").unwrap();
    array.store_metadata().unwrap();

    // Guard: ensure the on-disk metadata is genuinely native-v3 with no
    // `_ARRAY_DIMENSIONS` attribute, so this test actually exercises the gap.
    let zarr_json = std::fs::read_to_string(store_path.join("zarr.json")).unwrap();
    assert!(
        zarr_json.contains("\"zarr_format\": 3") || zarr_json.contains("\"zarr_format\":3"),
        "store must be zarr v3: {zarr_json}"
    );
    assert!(
        zarr_json.contains("dimension_names"),
        "store must carry native dimension_names: {zarr_json}"
    );
    assert!(
        !zarr_json.contains("_ARRAY_DIMENSIONS"),
        "store must NOT carry _ARRAY_DIMENSIONS: {zarr_json}"
    );

    // 1D coordinate arrays so the bbox can map to chunk indices.
    let lat_builder = ArrayBuilder::new(
        vec![50],
        DataType::Float64,
        vec![50].try_into().unwrap(),
        FillValue::from(0.0f64),
    );
    let lat_array = lat_builder.build(Arc::clone(&store), "/lat").unwrap();
    lat_array.store_metadata().unwrap();
    let lat_data: Vec<f64> = (0..50).map(|x| x as f64).collect();
    lat_array.store_chunk_elements(&[0], &lat_data).unwrap();

    let lon_builder = ArrayBuilder::new(
        vec![50],
        DataType::Float64,
        vec![50].try_into().unwrap(),
        FillValue::from(0.0f64),
    );
    let lon_array = lon_builder.build(Arc::clone(&store), "/lon").unwrap();
    lon_array.store_metadata().unwrap();
    let lon_data: Vec<f64> = (0..50).map(|x| x as f64).collect();
    lon_array.store_chunk_elements(&[0], &lon_data).unwrap();

    // Full extent: 50x50 / 5x5 = 100 chunks.
    let full_query = format!("SELECT * FROM plan_read_geo('{}')", store_path.display());
    let mut stmt = conn.prepare(&full_query)?;
    let mut rows = stmt.query([])?;
    let full_chunks: i64 = rows
        .next()?
        .expect("no rows for full extent")
        .get::<_, Option<i64>>(3)?
        .expect("null chunk count for full extent");
    assert_eq!(full_chunks, 100, "full extent should report all chunks");

    // Sub-bbox: lat 10..24 (chunk idx 2..4 => 3), lon 5..9 (chunk idx 1 => 1) => 3 chunks.
    let bbox_query = format!(
        "SELECT * FROM plan_read_geo('{}', lat_min=10, lat_max=24, lon_min=5, lon_max=9)",
        store_path.display()
    );
    let mut stmt = conn.prepare(&bbox_query)?;
    let mut rows = stmt.query([])?;
    let bbox_chunks: i64 = rows
        .next()?
        .expect("no rows for bbox")
        .get::<_, Option<i64>>(3)?
        .expect("null chunk count for bbox");

    assert_eq!(
        bbox_chunks, 3,
        "native-v3 bbox should prune to 3 chunks (was reading all {full_chunks})"
    );
    assert!(
        bbox_chunks < full_chunks,
        "pruning must engage on native v3: bbox={bbox_chunks} full={full_chunks}"
    );

    Ok(())
}
