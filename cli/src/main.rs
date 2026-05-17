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
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Export { db, query, output } => {
            println!("Exporting to Zarr...");
            println!("Database: {:?}", db);
            println!("Query: {}", query);
            println!("Output: {}", output);

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
