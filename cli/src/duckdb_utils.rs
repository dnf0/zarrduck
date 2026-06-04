use crate::config::S3Config;
use color_eyre::eyre::{eyre, Result as EyreResult, WrapErr};
use duckdb::Connection;

pub fn detect_columns(
    conn: &duckdb::Connection,
    table: &str,
) -> EyreResult<(String, String, String, String, bool)> {
    let mut stmt = conn
        .prepare(&format!("DESCRIBE \"{}\"", table.replace("\"", "\"\"")))
        .wrap_err_with(|| format!("Failed to describe table '{}'", table))?;

    let mut rows = stmt.query([])?;

    let mut columns = Vec::new();
    let mut time_is_numeric = false;

    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_type: String = row.get(1)?;
        let col_lower = col_name.to_lowercase();
        columns.push(col_lower.clone());

        if (col_lower.contains("time") || col_lower.contains("date"))
            && (col_type.contains("INT")
                || col_type.contains("DOUBLE")
                || col_type.contains("FLOAT"))
        {
            time_is_numeric = true;
        }
    }

    // Heuristics
    let time_col = columns
        .iter()
        .find(|c| c.contains("time") || c.contains("date"))
        .cloned()
        .ok_or_else(|| eyre!("Could not automatically detect a time column"))?;

    let lat_col = columns
        .iter()
        .find(|c| c.contains("lat") || c == &"y")
        .cloned()
        .ok_or_else(|| eyre!("Could not automatically detect a latitude column"))?;

    let lon_col = columns
        .iter()
        .find(|c| c.contains("lon") || c == &"x")
        .cloned()
        .ok_or_else(|| eyre!("Could not automatically detect a longitude column"))?;

    let val_col = columns
        .iter()
        .find(|&c| c != &time_col && c != &lat_col && c != &lon_col && c != "geom")
        .cloned()
        .ok_or_else(|| eyre!("Could not automatically detect a value column"))?;

    Ok((time_col, lat_col, lon_col, val_col, time_is_numeric))
}

pub fn auto_calculate_chunks(
    conn: &duckdb::Connection,
    table: &str,
) -> EyreResult<serde_json::Map<String, serde_json::Value>> {
    let mut stmt = conn.prepare(&format!("DESCRIBE \"{}\"", table.replace("\"", "\"\"")))?;
    let mut rows = stmt.query([])?;

    let mut map = serde_json::Map::new();
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_lower = col_name.to_lowercase();
        if col_lower == "x" || col_lower.contains("lon") {
            map.insert(col_name, serde_json::json!(100)); // Default spatial chunk
        } else if col_lower == "y" || col_lower.contains("lat") {
            map.insert(col_name, serde_json::json!(100));
        } else if col_lower.contains("time") || col_lower.contains("date") {
            map.insert(col_name, serde_json::json!(10)); // Default temporal chunk
        }
    }

    Ok(map)
}

pub fn load_geozarr_extension(conn: &Connection) -> EyreResult<()> {
    let ext_name = "eider_extension.duckdb_extension";

    let mut candidate_paths = vec![
        std::path::PathBuf::from(format!("./target/debug/{}", ext_name)),
        std::path::PathBuf::from(format!("../target/debug/{}", ext_name)),
    ];

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            candidate_paths.push(parent.join(ext_name));
            if let Some(grandparent) = parent.parent() {
                candidate_paths.push(grandparent.join(ext_name));
            }
        }
    }

    let ext_path = candidate_paths
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from(format!("../target/debug/{}", ext_name)));

    let ext_path_str = ext_path.to_string_lossy().into_owned();

    conn.execute(&format!("LOAD '{}'", ext_path_str), [])
        .wrap_err_with(|| format!("Failed to load extension at {}", ext_path_str))?;

    Ok(())
}

pub fn setup_duckdb(s3_config: Option<&S3Config>) -> EyreResult<Connection> {
    let config = duckdb::Config::default()
        .allow_unsigned_extensions()
        .wrap_err("Failed to configure unsigned extensions")?;
    let conn = Connection::open_in_memory_with_flags(config)
        .wrap_err("Failed to open in-memory DuckDB connection")?;

    load_geozarr_extension(&conn).wrap_err("Failed to load geozarr extension")?;

    inject_s3_secret(&conn, s3_config)?;

    Ok(conn)
}

pub fn inject_s3_secret(conn: &Connection, s3_config: Option<&S3Config>) -> EyreResult<()> {
    if let Some(s3) = s3_config {
        if s3.access_key.is_some()
            || s3.secret_key.is_some()
            || s3.region.is_some()
            || s3.endpoint.is_some()
        {
            let mut parts = vec!["TYPE S3".to_string()];
            if let Some(ak) = &s3.access_key {
                parts.push(format!("KEY_ID '{}'", ak.replace("'", "''")));
            }
            if let Some(sk) = &s3.secret_key {
                parts.push(format!("SECRET '{}'", sk.replace("'", "''")));
            }
            if let Some(r) = &s3.region {
                parts.push(format!("REGION '{}'", r.replace("'", "''")));
            }
            if let Some(e) = &s3.endpoint {
                parts.push(format!("ENDPOINT '{}'", e.replace("'", "''")));
            }

            let query = format!("CREATE SECRET ( {} )", parts.join(", "));
            conn.execute(&query, [])
                .wrap_err("Failed to inject S3 secret into DuckDB")?;
        }
    }
    Ok(())
}

pub fn format_pins(pins: &[String]) -> String {
    if pins.is_empty() {
        String::new()
    } else {
        format!(", pins := '{}'", pins.join(","))
    }
}

pub fn format_pins_where(pins: &[String]) -> String {
    if pins.is_empty() {
        String::new()
    } else {
        let conditions: Vec<String> = pins
            .iter()
            .map(|p| {
                let parts: Vec<&str> = p.splitn(2, '=').collect();
                if parts.len() == 2 {
                    format!("\"{}\" = {}", parts[0], parts[1])
                } else {
                    p.to_string() // Fallback if invalid format
                }
            })
            .collect();
        format!(" WHERE {}", conditions.join(" AND "))
    }
}
