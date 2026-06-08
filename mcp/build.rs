fn main() {
    // The bundled DuckDB (via the `duckdb` crate) references the Windows Restart
    // Manager (RmStartSession/RmEndSession/...) in `duckdb::AdditionalLockInfo`.
    // libduckdb-sys doesn't link it, so any target that links DuckDB directly —
    // the `eider-mcp` binary and the `protocol` integration test — needs
    // rstrtmgr explicitly. Mirrors cli/build.rs and extension/build.rs.
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-lib=dylib=rstrtmgr");
}
