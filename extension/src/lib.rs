pub mod metadata_vtab;
pub mod table_function;
pub mod vector_writer;
pub use metadata_vtab::ReadZarrMetadataVTab;
pub use table_function::{PlanReadGeoVTab, ReadGeoVTab};


#[cfg(feature = "loadable-extension")]
#[duckdb::duckdb_entrypoint_c_api(ext_name = "eider")]
fn init(conn: duckdb::Connection) -> duckdb::Result<()> {
    conn.register_table_function::<ReadGeoVTab>("read_geo")?;
    conn.register_table_function::<PlanReadGeoVTab>("plan_read_geo")?;
    conn.register_table_function::<ReadZarrMetadataVTab>("read_zarr_metadata")?;
    Ok(())
}
