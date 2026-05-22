pub mod metadata_vtab;
pub mod table_function;
pub mod vector_writer;
pub use metadata_vtab::ReadZarrMetadataVTab;
pub use table_function::{PlanReadZarrVTab, ReadZarrVTab};

#[cfg(feature = "loadable-extension")]
#[duckdb::duckdb_entrypoint_c_api]
fn init(conn: duckdb::Connection) -> duckdb::Result<()> {
    conn.register_table_function::<ReadZarrVTab>("read_zarr")?;
    conn.register_table_function::<PlanReadZarrVTab>("plan_read_zarr")?;
    conn.register_table_function::<ReadZarrMetadataVTab>("read_zarr_metadata")?;
    Ok(())
}
