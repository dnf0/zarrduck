use crate::{config::EiderConfig, duckdb_utils, ui::OutputMode};
use color_eyre::eyre::{eyre, Result as EyreResult, WrapErr};

pub async fn run_ingest(
    input_file: String,
    output_zarr_uri: String,
    chunks: Option<String>,
    value_column: Option<String>,
    mode: OutputMode,
    config: &EiderConfig,
) -> EyreResult<()> {
    if !std::path::Path::new(&input_file).exists() {
        return Err(eyre!("Input file '{}' does not exist.", input_file));
    }

    let conn = duckdb_utils::setup_duckdb(config.s3.as_ref())?;

    if mode != OutputMode::AgentJson {
        println!("Loading DuckDB spatial extension...");
    }
    conn.execute("INSTALL spatial", [])
        .wrap_err("Failed to install spatial extension")?;
    conn.execute("LOAD spatial", [])
        .wrap_err("Failed to load spatial extension")?;

    if mode != OutputMode::AgentJson {
        println!("Reading legacy file into DuckDB...");
    }

    // Create a view wrapping the ST_Read call to treat it as a table
    let view_query = format!(
        "CREATE VIEW temp_ingest AS SELECT * EXCLUDE (geom) FROM ST_Read('{}')",
        input_file.replace("'", "''")
    );
    conn.execute(&view_query, [])
        .wrap_err("Failed to execute ST_Read on input file")?;

    let mut final_chunks = duckdb_utils::auto_calculate_chunks(&conn, "temp_ingest")?;

    if let Some(user_chunks_str) = chunks {
        let user_chunks: serde_json::Value = serde_json::from_str(&user_chunks_str)
            .wrap_err("Failed to parse user --chunks flag as JSON")?;

        if let Some(user_obj) = user_chunks.as_object() {
            for (k, v) in user_obj {
                final_chunks.insert(k.clone(), v.clone());
            }
        } else {
            return Err(eyre!("--chunks must be a JSON object"));
        }
    }

    if mode != OutputMode::AgentJson {
        println!(
            "Calculated chunk shape: {}",
            serde_json::Value::Object(final_chunks.clone())
        );
    }
    let val_col = if let Some(vc) = value_column {
        vc
    } else {
        let mut stmt = conn.prepare("DESCRIBE temp_ingest")?;
        let mut rows = stmt.query([])?;
        let mut fallback_col = "value".to_string();
        let exclude_cols = [
            "geom",
            "x",
            "y",
            "lon",
            "lat",
            "longitude",
            "latitude",
            "time",
            "t",
        ];
        while let Some(row) = rows.next()? {
            let col_name: String = row.get(0)?;
            let col_lower = col_name.to_lowercase();
            if !exclude_cols.contains(&col_lower.as_str()) {
                fallback_col = col_name;
                break;
            }
        }
        fallback_col
    };
    let query = "SELECT * FROM temp_ingest";

    if mode != OutputMode::AgentJson {
        println!("Starting streaming export to Zarr...");
    }

    crate::export::run_export(
        &conn,
        query,
        &output_zarr_uri,
        &val_col,
        Some(serde_json::Value::Object(final_chunks).to_string()),
        mode == OutputMode::AgentJson,
    )
    .await?;

    if mode == OutputMode::AgentJson {
        println!(r#"{{"status": "success", "uri": "{}"}}"#, output_zarr_uri);
    } else {
        println!("Ingestion complete! Data available at {}", output_zarr_uri);
    }

    Ok(())
}
