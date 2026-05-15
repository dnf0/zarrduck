use duckdb::{Connection, Result};

#[duckdb::duckdb_entrypoint_c_api]
fn init(_conn: Connection) -> Result<()> {
    // Basic initialization empty for now
    Ok(())
}
