use duckdb::{Connection, Result};
use std::fs;

#[test]
fn read_geo_stac_asset_returns_rows() -> Result<()> {
    std::env::set_var("GEOZARR_ALLOW_PATH", "/");
    // The committed STAC Item fixture lives in the geozarr_core crate's fixtures dir.
    let stac_path = format!(
        "{}/../geozarr_core/tests/fixtures/stac_item.json",
        env!("CARGO_MANIFEST_DIR")
    );

    if fs::metadata(&stac_path).is_err() {
        println!("stac_item.json not found, skipping e2e test");
        return Ok(());
    }

    // Load the extension via native Rust API to bypass duckdb dynamic extension version mismatches
    let conn = Connection::open_in_memory()?;
    conn.register_table_function::<eider::ReadGeoVTab>("read_geo")?;

    // Select a specific asset from the STAC Item via the `asset` named parameter.
    let query = format!(
        "SELECT COUNT(*) FROM read_geo('{}', asset := 'band_uncompressed')",
        stac_path
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let count: i64 = row.get(0)?;
        println!(
            "Successfully read {} rows from STAC asset via VirtualStore!",
            count
        );
        assert!(count > 0);
    } else {
        panic!("No rows returned");
    }

    Ok(())
}
