use crate::config::EiderConfig;
use crate::duckdb_utils;
use crate::ui;
use crate::ui::OutputMode;
use color_eyre::eyre::{eyre, Result as EyreResult};
use owo_colors::OwoColorize;

pub async fn run_info(
    uri: String,
    pin: Vec<String>,
    mode: OutputMode,
    config: &EiderConfig,
) -> EyreResult<()> {
    let uri = ui::prompt_zarr_uri(&uri, mode).await?;
    let conn = duckdb_utils::setup_duckdb(config.s3.as_ref())?;
    let escaped_uri = uri.replace('\'', "''");
    let pins_str = duckdb_utils::format_pins(&pin);
    let query = format!(
        "SELECT array_shape, chunk_shape, data_type, crs FROM read_zarr_metadata('{}'{})",
        escaped_uri, pins_str
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    if let Some(row) = rows.next()? {
        let array_shape: String = row.get(0)?;
        let chunk_shape: String = row.get(1)?;
        let data_type: String = row.get(2)?;
        let crs: String = row.get(3)?;

        if mode == OutputMode::AgentJson {
            let json_out = serde_json::json!({
                "uri": uri,
                "array_shape": array_shape,
                "chunk_shape": chunk_shape,
                "data_type": data_type,
                "crs": crs
            });
            println!("{}", json_out);
        } else {
            let title = if mode.is_human() {
                "GeoZarr Dataset Info".bold().to_string()
            } else {
                "### GeoZarr Dataset Info".to_string()
            };
            println!("{}", title);
            println!(
                "{}: {}",
                ui::format_key("URI", mode),
                ui::format_value(&uri, mode)
            );
            println!(
                "{}: {}",
                ui::format_key("Shape", mode),
                ui::format_value(&array_shape, mode)
            );
            println!(
                "{}: {}",
                ui::format_key("Chunks", mode),
                ui::format_value(&chunk_shape, mode)
            );
            println!(
                "{}: {}",
                ui::format_key("Type", mode),
                ui::format_value(&data_type, mode)
            );
            println!(
                "{}: {}",
                ui::format_key("CRS", mode),
                ui::format_value(&crs, mode)
            );
        }
    } else {
        return Err(eyre!("Failed to read metadata for {}", uri));
    }

    Ok(())
}
