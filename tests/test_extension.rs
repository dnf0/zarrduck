use std::path::Path;

#[test]
fn test_extension_compiles() {
    #[cfg(target_os = "macos")]
    let lib_path = "target/debug/libgeozarr.dylib";
    #[cfg(target_os = "linux")]
    let lib_path = "target/debug/libgeozarr.so";
    
    // We just verify the library was successfully built.
    // Loading it dynamically into an engine requires `cargo-duckdb-ext-tools`
    // to append the DuckDB extension metadata footer, which is handled
    // by the build/packaging step, not the raw unit tests.
    assert!(Path::new(lib_path).exists(), "Extension shared library not found. Run cargo build first.");
}
