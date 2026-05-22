mod commands;
mod config;
mod duckdb_utils;
mod export;
mod plot;
mod stac;
mod ui;

use config::ZarrduckConfig;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use color_eyre::eyre::Result as EyreResult;

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}

#[derive(Parser)]
#[command(name = "zarrduck")]
#[command(about = "Agentic Spatial Data Engine for GeoZarr and DuckDB", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format (table or json)
    #[arg(global = true, long)]
    output: Option<OutputFormat>,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover dataset metadata
    Info {
        /// The Zarr array URI
        uri: String,
        /// Pins for dimensions (e.g. --pin time=0)
        #[arg(long, value_name = "DIM=INDEX", action = clap::ArgAction::Append)]
        pin: Vec<String>,
    },
    /// Extract Zarr data intersecting with vector polygons
    Extract {
        /// The Zarr array URI
        zarr_uri: String,
        /// Path to the vector boundaries (GeoJSON, Shapefile)
        vector_path: String,
        /// Output DuckDB database file
        #[arg(long)]
        out: Option<String>,
        /// Bypass confirmation prompts
        #[arg(short = 'y', long)]
        yes: bool,
        /// Pins for dimensions (e.g. --pin time=0)
        #[arg(long, value_name = "DIM=INDEX", action = clap::ArgAction::Append)]
        pin: Vec<String>,
    },
    /// Open an interactive DuckDB shell loaded with the data
    Shell {
        /// The DuckDB database file to open
        db_path: String,
    },
    /// Export DuckDB query results to a Zarr array
    Export {
        /// Path to the DuckDB database file (or leave empty for in-memory)
        #[arg(long)]
        db: Option<String>,

        /// The SQL query to execute
        #[arg(long)]
        query: String,

        /// The destination path for the Zarr array (e.g., s3://bucket/output.zarr)
        #[arg(long)]
        output: String,

        /// The column containing the actual values (all others are coordinates)
        #[arg(long)]
        value_column: String,

        /// Optional JSON mapping of dimension name to chunk size (e.g. '{"time": 10}')
        #[arg(long)]
        chunks: Option<String>,
    },
    /// Generate shell completion scripts
    Completions {
        /// The shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Search a STAC API for GeoZarr assets
    Search {
        /// The STAC API URL (e.g., https://planetarycomputer.microsoft.com/api/stac/v1)
        #[arg(long)]
        api: Option<String>,

        /// The collection ID to search (e.g., era5-pds)
        #[arg(long)]
        collection: Option<String>,
        /// Bounding box (min_lon, min_lat, max_lon, max_lat)
        #[arg(long, allow_hyphen_values = true)]
        bbox: Option<String>,

        /// Datetime range (e.g., 2020-01-01T00:00:00Z/2020-12-31T23:59:59Z)
        #[arg(long)]
        datetime: Option<String>,
    },
    /// Temporally resample extracted GeoZarr data
    Resample {
        /// The input DuckDB file containing the 'extracted_data' table
        input_db: String,

        /// The output DuckDB file to save the resampled data
        output_db: String,

        /// The temporal frequency (e.g., month, year, day)
        #[arg(long)]
        freq: Option<String>,
        /// The aggregate function to apply (e.g., avg, sum, max)
        #[arg(long)]
        agg: Option<String>,
    },
    /// Plot data from a local DuckDB file
    Plot {
        /// The DuckDB database file
        db_path: String,

        /// Type of plot (hist, heatmap, line)
        #[arg(long, value_enum)]
        plot_type: Option<plot::PlotType>,

        /// The table to query
        #[arg(long, default_value = "extracted_data")]
        table: String,

        /// The value column to aggregate (auto-detected if omitted)
        #[arg(long)]
        value: Option<String>,

        /// Optional column to group by
        #[arg(long)]
        group_by: Option<String>,

        /// Pins for dimensions (e.g. --pin time=0)
        #[arg(long, value_name = "DIM=INDEX", action = clap::ArgAction::Append)]
        pin: Vec<String>,
    },
    /// Convert legacy spatial files (NetCDF, GeoTIFF, CSV) to GeoZarr
    Ingest {
        /// The local file to ingest
        input_file: String,

        /// The destination Zarr URI
        output_zarr_uri: String,

        /// Optional JSON string to override automatic chunk sizes (e.g., '{"time": 30}')
        #[arg(long)]
        chunks: Option<String>,

        /// Optional name for the value column (defaults to "value")
        #[arg(long)]
        value_column: Option<String>,
    },
}

#[tokio::main]
async fn main() -> EyreResult<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let config = ZarrduckConfig::load().unwrap_or(ZarrduckConfig {
        output_format: None,
        default_out: None,
        local_stac: None,
        s3: None,
    });

    let is_json = cli
        .output
        .as_ref()
        .map(|o| *o == OutputFormat::Json)
        .unwrap_or_else(|| config.output_format.as_deref() == Some("json"));

    if let Err(e) = run_cli(cli, config).await {
        if is_json {
            // Build error chain string
            let error_msgs: Vec<String> = e.chain().map(|c| c.to_string()).collect();
            let json_err = serde_json::json!({
                "status": "error",
                "message": error_msgs.join(": ")
            });
            println!("{}", json_err);
            std::process::exit(1);
        } else {
            // Return error to let color-eyre format it
            return Err(e);
        }
    }

    Ok(())
}

async fn run_cli(mut cli: Cli, config: ZarrduckConfig) -> EyreResult<()> {
    let resolved_output = cli
        .output
        .clone()
        .or_else(|| {
            config.output_format.as_deref().and_then(|s| match s {
                "json" => Some(OutputFormat::Json),
                "table" => Some(OutputFormat::Table),
                _ => None,
            })
        })
        .unwrap_or(OutputFormat::Table);

    // Update cli struct so nested commands can just use it
    cli.output = Some(resolved_output.clone());

    execute_command(cli.command, resolved_output, config).await
}

#[allow(clippy::too_many_lines)]
async fn execute_command(
    command: Commands,
    resolved_output: OutputFormat,
    config: ZarrduckConfig,
) -> EyreResult<()> {
    match command {
        Commands::Info { uri, pin } => {
            commands::info::run_info(uri, pin, &resolved_output, &config).await?;
        }
        Commands::Extract {
            zarr_uri,
            vector_path,
            out,
            yes,
            pin,
        } => {
            commands::extract::run_extract(
                zarr_uri,
                vector_path,
                out,
                yes,
                pin,
                &resolved_output,
                &config,
            )
            .await?;
        }
        Commands::Shell { db_path } => {
            commands::shell::run_shell(db_path)?;
        }
        Commands::Export {
            db,
            query,
            output,
            value_column,
            chunks,
        } => {
            commands::export_cmd::run_export_cmd(
                db,
                query,
                output,
                value_column,
                chunks,
                &resolved_output,
                &config,
            )
            .await?;
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
        Commands::Search {
            api,
            collection,
            bbox,
            datetime,
        } => {
            commands::search::run_search(
                api,
                collection,
                bbox,
                datetime,
                &resolved_output,
                &config,
            )
            .await?;
        }
        Commands::Resample {
            input_db,
            output_db,
            freq,
            agg,
        } => {
            commands::resample::run_resample(input_db, output_db, freq, agg, &resolved_output)?;
        }
        Commands::Ingest {
            input_file,
            output_zarr_uri,
            chunks,
            value_column,
        } => {
            commands::ingest::run_ingest(
                input_file,
                output_zarr_uri,
                chunks,
                value_column,
                &resolved_output,
                &config,
            )
            .await?;
        }
        Commands::Plot {
            db_path,
            plot_type,
            table,
            value,
            group_by,
            pin,
        } => {
            plot::run_plot(
                &db_path,
                plot_type,
                &table,
                value.as_deref(),
                group_by.as_deref(),
                pin,
            )?;
        }
    }

    Ok(())
}
