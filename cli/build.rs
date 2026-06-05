fn main() {
    // The bundled DuckDB (via the `duckdb` crate) references the Windows Restart
    // Manager (RmStartSession/RmEndSession/...) in `duckdb::AdditionalLockInfo`.
    // libduckdb-sys doesn't link it, so any target that links DuckDB directly —
    // notably the integration-test binaries that open a `duckdb::Connection` —
    // needs rstrtmgr explicitly. Mirrors extension/build.rs.
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=dylib=rstrtmgr");
}
