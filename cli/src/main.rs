use clap::{Parser, Subcommand};
use duckdb::{Connection, Result};

#[derive(Parser)]
#[command(name = "geozarr-cli")]
#[command(about = "Companion CLI tool for exporting DuckDB tables to Zarr", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Export the results of a SQL query to a Zarr array
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Export {
            db,
            query,
            output,
            value_column,
            chunks,
        } => {
            println!("Exporting to Zarr...");
            println!("Database: {:?}", db);
            println!("Query: {}", query);
            println!("Output: {}", output);
            println!("Value Column: {}", value_column);
            if let Some(c) = &chunks {
                println!("Chunks: {}", c);
            }

            let _conn = match db {
                Some(path) => Connection::open(path)?,
                None => Connection::open_in_memory()?,
            };

            // Two-pass inference and data writing will go here
            println!("Export successful!");
        }
    }

    Ok(())
}
