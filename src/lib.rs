use duckdb::{Connection, Result};

mod table_function;
use table_function::ReadZarrVTab;

#[duckdb::duckdb_entrypoint_c_api]
fn init(conn: Connection) -> Result<()> {
    conn.register_table_function::<ReadZarrVTab>("read_zarr")?;
    Ok(())
}
