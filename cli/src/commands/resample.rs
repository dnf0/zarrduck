use crate::{duckdb_utils, OutputFormat};
use color_eyre::eyre::{eyre, Result as EyreResult, WrapErr};
use duckdb::Connection;

fn get_freq_and_agg(
    freq: Option<String>,
    agg: Option<String>,
    resolved_output: &OutputFormat,
) -> EyreResult<(String, String)> {
    let selected_freq = if let Some(f) = freq {
        f
    } else {
        if resolved_output == &OutputFormat::Json {
            return Err(eyre!("--freq is required when using --output=json"));
        }
        inquire::Select::new(
            "Select temporal resampling frequency:",
            vec!["hour", "day", "week", "month", "year"],
        )
        .prompt()?
        .to_string()
    };

    let selected_agg = if let Some(a) = agg {
        a
    } else {
        if resolved_output == &OutputFormat::Json {
            return Err(eyre!("--agg is required when using --output=json"));
        }
        inquire::Select::new(
            "Select aggregation function:",
            vec![
                "avg", "min", "max", "sum", "count", "median", "mode", "stddev", "variance",
            ],
        )
        .prompt()?
        .to_string()
    };

    let allowed_aggs = [
        "sum", "avg", "min", "max", "count", "mean", "median", "mode", "stddev", "variance",
    ];
    if !allowed_aggs.contains(&selected_agg.to_lowercase().as_str()) {
        return Err(eyre!(
            "Invalid aggregation function: '{}'. Allowed: {:?}",
            selected_agg,
            allowed_aggs
        ));
    }

    Ok((selected_freq, selected_agg))
}

pub fn run_resample(
    input_db: String,
    output_db: String,
    freq: Option<String>,
    agg: Option<String>,
    resolved_output: &OutputFormat,
) -> EyreResult<()> {
    let (selected_freq, selected_agg) = get_freq_and_agg(freq, agg, resolved_output)?;

    if !std::path::Path::new(&input_db).exists() {
        return Err(eyre!("Input database '{}' does not exist.", input_db));
    }

    let input_conn = Connection::open(&input_db)
        .wrap_err_with(|| format!("Failed to open input database '{}'", input_db))?;

    let (time_col, lat_col, lon_col, val_col, time_is_numeric) =
        duckdb_utils::detect_columns(&input_conn, "extracted_data")?;

    if resolved_output != &OutputFormat::Json {
        println!(
            "Detected schema: Time='{}' (numeric={}), Spatial='{}', '{}', Value='{}'",
            time_col, time_is_numeric, lat_col, lon_col, val_col
        );
    }

    // Just close the input connection so we don't lock the file for the next step
    drop(input_conn);

    // Overwrite protection for output db
    if std::path::Path::new(&output_db).exists() {
        if resolved_output == &OutputFormat::Json {
            return Err(eyre!(
                "Output database '{}' already exists. Aborting.",
                output_db
            ));
        } else {
            let ans =
                inquire::Confirm::new(&format!("File '{}' already exists. Overwrite?", output_db))
                    .with_default(false)
                    .prompt()
                    .wrap_err("Failed to read user input")?;

            if !ans {
                println!("Aborting resampling.");
                return Ok(());
            }
            std::fs::remove_file(&output_db)
                .wrap_err_with(|| format!("Failed to delete '{}'", output_db))?;
        }
    }

    let conn = Connection::open(&output_db)
        .wrap_err_with(|| format!("Failed to open output database '{}'", output_db))?;

    let spinner = if resolved_output != &OutputFormat::Json {
        let pb =
            indicatif::ProgressBar::with_draw_target(None, indicatif::ProgressDrawTarget::stdout());
        pb.set_style(
            indicatif::ProgressStyle::default_spinner()
                .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("Resampling time-series data...");
        Some(pb)
    } else {
        None
    };

    conn.execute(&format!("ATTACH '{}' AS source_db", input_db), [])
        .wrap_err("Failed to attach input database")?;

    let time_expr = if time_is_numeric {
        format!("to_timestamp(CAST({} AS BIGINT))", time_col)
    } else {
        time_col.clone()
    };

    let query = format!(
        "CREATE TABLE resampled_data AS
         SELECT
             date_trunc('{}', {}) as {},
             {}, {},
             {}({}) as value
         FROM source_db.extracted_data
         GROUP BY 1, 2, 3",
        selected_freq.replace("'", "''"),
        time_expr,
        time_col,
        lat_col,
        lon_col,
        selected_agg,
        val_col
    );

    // Note: Since this is a blocking call, we run it directly on this thread. The tokio runtime isn't heavily needed here since it's local.
    conn.execute(&query, [])
        .wrap_err("Resampling query failed")?;

    if let Some(pb) = spinner {
        pb.finish_and_clear();
        println!("Resampling complete!");
    }

    if resolved_output == &OutputFormat::Json {
        println!(r#"{{"status": "success", "db": "{}"}}"#, output_db);
    } else {
        println!("Data saved to table 'resampled_data' in {}", output_db);
        println!("Run `zarrduck shell {}` to explore it.", output_db);
    }

    Ok(())
}
