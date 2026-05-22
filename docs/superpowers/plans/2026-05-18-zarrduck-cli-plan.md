# Zarrduck CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the CLI to `zarrduck` and implement a suite of agent-friendly commands (`info`, `extract`, `shell`) that abstract away complex spatial SQL operations.

**Architecture:** We will use `clap` to structure the multi-command CLI. The CLI will act as a wrapper around an embedded DuckDB connection. It will install and load necessary extensions (`zarrduck`, `spatial`), construct the complex SQL for metadata fetching or spatial joins, execute it, and format the output (either as human-readable tables or agent-friendly JSON).

**Tech Stack:** Rust, `clap`, `tokio`, `duckdb`, `serde_json`

---

### Task 1: Rename Crate and Scaffold CLI Structure

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Modify Cargo definitions**

Update `cli/Cargo.toml` to rename the package to `zarrduck`:
```toml
[package]
name = "zarrduck"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4.4", features = ["derive"] }
duckdb = { version = "1.10502.0", features = ["bundled"] }
tokio = { version = "1.0", features = ["full"] }
opendal = { version = "0.48", features = ["services-s3", "services-http"] }
zarrs = { version = "0.16.4", features = ["opendal", "async"] }
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
```

Update root `Cargo.toml` if necessary (though the folder is still `cli`, the package name changes).

- [ ] **Step 2: Scaffold `clap` commands in `main.rs`**

Replace the top of `cli/src/main.rs` to define the new commands:

```rust
use clap::{Parser, Subcommand};
use duckdb::{Connection, Result};
use std::process::Command;

#[derive(Parser)]
#[command(name = "zarrduck")]
#[command(about = "Agentic Spatial Data Engine for GeoZarr and DuckDB", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format (table or json)
    #[arg(global = true, long, default_value = "table")]
    output: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover dataset metadata
    Info {
        /// The Zarr array URI
        uri: String,
    },
    /// Extract Zarr data intersecting with vector polygons
    Extract {
        /// The Zarr array URI
        zarr_uri: String,
        /// Path to the vector boundaries (GeoJSON, Shapefile)
        vector_path: String,
        /// Output DuckDB database file
        #[arg(long)]
        out: String,
    },
    /// Open an interactive DuckDB shell loaded with the data
    Shell {
        /// The DuckDB database file to open
        db_path: String,
    },
    /// Export DuckDB query results to a Zarr array
    Export {
        #[arg(long)]
        db: Option<String>,
        #[arg(long)]
        query: String,
        #[arg(long)]
        output: String,
        #[arg(long)]
        value_column: String,
        #[arg(long)]
        chunks: Option<String>,
    },
}
```

- [ ] **Step 3: Update the `main` match block**

```rust
// In cli/src/main.rs inside main()
    match cli.command {
        Commands::Info { uri } => {
            println!("Info command for {}", uri);
            // TODO in Task 2
        }
        Commands::Extract { zarr_uri, vector_path, out } => {
            println!("Extracting {} using {} to {}", zarr_uri, vector_path, out);
            // TODO in Task 3
        }
        Commands::Shell { db_path } => {
            println!("Opening shell for {}", db_path);
            // TODO in Task 4
        }
        Commands::Export { db, query, output, value_column, chunks } => {
            // Keep existing export logic here...
            println!("Exporting...");
        }
    }
```

- [ ] **Step 4: Verify Compilation**

Run: `cargo build -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs Cargo.toml
git commit -m "feat: rename cli to zarrduck and scaffold info, extract, and shell commands"
```

---

### Task 2: Implement the `info` Command

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write helper for DuckDB setup**

Add a function to initialize an in-memory DuckDB connection and load our extension:
```rust
fn setup_duckdb() -> Result<Connection, Box<dyn std::error::Error>> {
    let conn = Connection::open_in_memory()?;

    // We need to construct the path to our compiled extension
    let ext_path = if cfg!(target_os = "windows") {
        "../target/debug/geozarr.duckdb_extension"
    } else {
        "../target/debug/libgeozarr.duckdb_extension"
    };

    conn.execute("SET allow_unsigned_extensions = true", [])?;
    conn.execute(&format!("LOAD '{}'", ext_path), [])?;
    Ok(conn)
}
```

- [ ] **Step 2: Implement `info` logic**

Inside the `Commands::Info` match block:

```rust
        Commands::Info { uri } => {
            let conn = setup_duckdb()?;
            let query = format!("SELECT array_shape, chunk_shape, data_type, crs FROM read_zarr_metadata('{}')", uri);

            let mut stmt = conn.prepare(&query)?;
            let mut rows = stmt.query([])?;

            if let Some(row) = rows.next()? {
                let array_shape: String = row.get(0)?;
                let chunk_shape: String = row.get(1)?;
                let data_type: String = row.get(2)?;
                let crs: String = row.get(3)?;

                if cli.output == "json" {
                    let json_out = serde_json::json!({
                        "uri": uri,
                        "array_shape": array_shape,
                        "chunk_shape": chunk_shape,
                        "data_type": data_type,
                        "crs": crs
                    });
                    println!("{}", json_out.to_string());
                } else {
                    println!("GeoZarr Dataset Info:");
                    println!("URI: {}", uri);
                    println!("Shape: {}", array_shape);
                    println!("Chunks: {}", chunk_shape);
                    println!("Type: {}", data_type);
                    println!("CRS: {}", crs);
                }
            } else {
                eprintln!("Failed to read metadata for {}", uri);
                std::process::exit(1);
            }
        }
```

- [ ] **Step 3: Test implementation**

Run: `cargo run -p zarrduck -- info "s3://invalid/test" --output json`
Expected: It will fail at the extension load or runtime query because it's an invalid S3 path, but it should successfully compile and attempt the query.

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: implement zarrduck info command for metadata discovery"
```

---

### Task 3: Implement the `extract` Command (Vector-Raster Join)

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write `extract` logic**

Inside the `Commands::Extract` match block. We will create a local `.duckdb` database, load the spatial and geozarr extensions, and run a `CREATE TABLE` spatial join query.

```rust
        Commands::Extract { zarr_uri, vector_path, out } => {
            let conn = Connection::open(&out)?;

            // Load extensions
            conn.execute("SET allow_unsigned_extensions = true", [])?;
            let ext_path = if cfg!(target_os = "windows") {
                "../target/debug/geozarr.duckdb_extension"
            } else {
                "../target/debug/libgeozarr.duckdb_extension"
            };
            conn.execute(&format!("LOAD '{}'", ext_path), [])?;

            // Install and load official spatial extension
            println!("Loading DuckDB spatial extension...");
            conn.execute("INSTALL spatial", [])?;
            conn.execute("LOAD spatial", [])?;

            println!("Extracting data... This may take a while depending on the bounding box.");

            // The magic query: Create a table by joining the GeoZarr pixels that intersect the vector polygons
            let query = format!(
                "CREATE TABLE extracted_data AS
                 SELECT z.*, v.* EXCLUDE (geom)
                 FROM read_zarr('{}') z, ST_Read('{}') v
                 WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat))",
                zarr_uri, vector_path
            );

            match conn.execute(&query, []) {
                Ok(_) => {
                    if cli.output == "json" {
                        println!(r#"{{"status": "success", "db": "{}"}}"#, out);
                    } else {
                        println!("Extraction complete! Data saved to table 'extracted_data' in {}", out);
                        println!("Run `zarrduck shell {}` to explore it.", out);
                    }
                },
                Err(e) => {
                    eprintln!("Extraction failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
```

- [ ] **Step 2: Verify Compilation**

Run: `cargo build -p zarrduck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: implement zarrduck extract command for zonal statistics"
```

---

### Task 4: Implement the `shell` Command

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write `shell` logic**

Inside the `Commands::Shell` match block. We will spawn the actual `duckdb` binary process, passing it init commands to ensure the extensions are loaded so the user can query natively.

```rust
        Commands::Shell { db_path } => {
            let ext_path = if cfg!(target_os = "windows") {
                "../target/debug/geozarr.duckdb_extension"
            } else {
                "../target/debug/libgeozarr.duckdb_extension"
            };

            let init_commands = format!(
                "SET allow_unsigned_extensions = true; LOAD '{}'; INSTALL spatial; LOAD spatial;",
                ext_path
            );

            println!("Starting DuckDB shell...");
            let status = Command::new("duckdb")
                .arg(&db_path)
                .arg("-cmd")
                .arg(&init_commands)
                .status();

            match status {
                Ok(s) if s.success() => {},
                Ok(s) => eprintln!("DuckDB shell exited with status: {}", s),
                Err(e) => eprintln!("Failed to launch 'duckdb' CLI. Is it installed in your PATH? Error: {}", e),
            }
        }
```

- [ ] **Step 2: Verify Compilation**

Run: `cargo build -p zarrduck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: implement zarrduck shell command to launch interactive duckdb repl"
```
