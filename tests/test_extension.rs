use std::path::Path;

#[test]
fn test_read_zarr_function_compiles() {
    let ext_path = "target/debug/duckdb_geozarr.duckdb_extension";
    
    // We just verify the extension file was successfully packaged by `cargo duckdb-ext build`.
    // Loading it dynamically into the host duckdb CLI is not tested here because
    // DuckDB extensions are strictly tied to the exact minor version (v1.1.x vs v1.5.x)
    // and will panic on load if there's a mismatch.
    assert!(
        Path::new(ext_path).exists(),
        "Extension file not found. Please run `cargo duckdb-ext build` before `cargo test`"
    );
}
