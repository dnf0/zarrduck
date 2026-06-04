use crate::config::EiderConfig;
use crate::duckdb_utils;
use crate::ui::OutputMode;
use color_eyre::eyre::Result as EyreResult;
use duckdb::Connection;

pub async fn run_export_cmd(
    db: Option<String>,
    query: String,
    output: String,
    value_column: String,
    chunks: Option<String>,
    mode: OutputMode,
    config: &EiderConfig,
) -> EyreResult<()> {
    let conn = if let Some(db_path) = db {
        let db_config = duckdb::Config::default().allow_unsigned_extensions()?;
        let c = Connection::open_with_flags(db_path, db_config)?;
        duckdb_utils::load_geozarr_extension(&c)?;
        duckdb_utils::inject_s3_secret(&c, config.s3.as_ref())?;
        c
    } else {
        duckdb_utils::setup_duckdb(config.s3.as_ref())?
    };

    crate::export::run_export(
        &conn,
        &query,
        &output,
        &value_column,
        chunks,
        mode == OutputMode::AgentJson,
    )
    .await?;

    Ok(())
}
