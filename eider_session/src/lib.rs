//! Shared DuckDB session setup for eider tools (CLI and MCP), so extension
//! loading and the read sandbox can't drift between them.
use color_eyre::eyre::{Result as EyreResult, WrapErr};
use duckdb::Connection;

/// Load the eider loadable extension. Honors `EIDER_EXTENSION_PATH`, else
/// discovers `eider.duckdb_extension` next to the binary or under target/debug.
pub fn load_eider_extension(conn: &Connection) -> EyreResult<()> {
    let ext_name = "eider.duckdb_extension";
    if let Ok(explicit) = std::env::var("EIDER_EXTENSION_PATH") {
        if !explicit.is_empty() {
            return conn
                .execute(&format!("LOAD '{}'", explicit.replace('\'', "''")), [])
                .map(|_| ())
                .wrap_err_with(|| format!("Failed to load extension at {explicit}"));
        }
    }
    let mut candidates = vec![
        std::path::PathBuf::from(format!("./target/debug/{ext_name}")),
        std::path::PathBuf::from(format!("../target/debug/{ext_name}")),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent() {
            candidates.push(p.join(ext_name));
            if let Some(gp) = p.parent() {
                candidates.push(gp.join(ext_name));
            }
        }
    }
    let path = candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(format!("../target/debug/{ext_name}")));
    let s = path.to_string_lossy().replace('\'', "''");
    conn.execute(&format!("LOAD '{s}'"), [])
        .wrap_err_with(|| format!("Failed to load eider extension at {s}"))?;
    Ok(())
}

/// Open an in-memory DuckDB connection with the eider extension loaded.
pub fn open_connection() -> EyreResult<Connection> {
    let config = duckdb::Config::default()
        .allow_unsigned_extensions()
        .wrap_err("config unsigned extensions")?;
    let conn = Connection::open_in_memory_with_flags(config).wrap_err("open in-memory duckdb")?;
    load_eider_extension(&conn).wrap_err("load eider extension")?;
    Ok(conn)
}

/// Open a session for the MCP/analytics use case: eider + the DuckDB `spatial`
/// extension loaded. `spatial` provides ST_Read / ST_Contains / ST_Intersects.
pub fn open_session() -> EyreResult<Connection> {
    let conn = open_connection()?;
    conn.execute_batch("INSTALL spatial; LOAD spatial;")
        .wrap_err("install/load spatial extension")?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_has_eider_and_spatial() {
        if std::env::var("EIDER_EXTENSION_PATH").is_err()
            && !std::path::Path::new("target/debug/eider.duckdb_extension").exists()
            && !std::path::Path::new("../target/debug/eider.duckdb_extension").exists()
        {
            eprintln!("skip: eider extension not built");
            return;
        }
        let conn = open_session().unwrap();
        // eider function present:
        conn.execute_batch("SELECT 1").unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM duckdb_functions() WHERE function_name='read_geo'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(n >= 1, "read_geo should be registered");
        // spatial present:
        let s: i64 = conn
            .query_row(
                "SELECT count(*) FROM duckdb_functions() WHERE lower(function_name)='st_contains'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(s >= 1, "spatial ST_Contains should be registered");
    }
}
