pub mod metadata;
pub mod metadata_vtab;
pub mod table_function;
pub use metadata_vtab::ReadZarrMetadataVTab;
pub use table_function::ReadZarrVTab;

#[cfg(feature = "loadable-extension")]
#[duckdb::duckdb_entrypoint_c_api]
fn init(conn: duckdb::Connection) -> duckdb::Result<()> {
    conn.register_table_function::<ReadZarrVTab>("read_zarr")?;
    conn.register_table_function::<ReadZarrMetadataVTab>("read_zarr_metadata")?;
    Ok(())
}
