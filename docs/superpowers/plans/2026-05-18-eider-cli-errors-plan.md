# Eider CLI Error Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate `color-eyre` to provide rich error diagnostics for human users while preserving strict JSON error formatting when running in agent mode.

**Architecture:** We will add `color-eyre` and refactor the core CLI execution logic into a `run_cli` helper returning `color_eyre::Result<()>`. The `main` function will intercept any errors from this helper. If `--output=json` is active, it will format the error chain as a JSON payload and exit; otherwise, it will bubble the error up to `color-eyre` for beautiful terminal rendering. Finally, we will replace manual `eprintln!` calls with `.wrap_err` to provide better context.

**Tech Stack:** Rust, `color-eyre`, `serde_json`

---

### Task 1: Add Dependencies and Refactor `main` Entry Point

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add `color-eyre` dependency**

Update `cli/Cargo.toml`:
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
duckdb = { version = "1.10502.0", features = ["bundled"] }
tokio = { version = "1.0", features = ["full"] }
opendal = { version = "0.48", features = ["services-s3", "services-http"] }
zarrs = { version = "0.16.4", features = ["opendal", "async"] }
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
color-eyre = "0.6"
```

- [ ] **Step 2: Refactor `main.rs` signature and `run_cli` helper**

In `cli/src/main.rs`, update the imports:
```rust
use clap::{Parser, Subcommand, ValueEnum};
use duckdb::{Connection, Result};
use color_eyre::eyre::{eyre, WrapErr, Result as EyreResult};
```

Move the `tokio::main` macro and the entire CLI logic from `main` to a new async `run_cli` function returning `EyreResult<()>`:
```rust
async fn run_cli() -> EyreResult<()> {
    let cli = Cli::parse();
    // [Keep the exact existing match cli.command block but update returns]
```
Wait, we need `cli` in `main` to check `cli.output`. Let's parse `cli` in `main()` and pass it to `run_cli(cli)`.

```rust
async fn run_cli(cli: Cli) -> EyreResult<()> {
    match cli.command {
        Commands::Info { uri } => {
// ... existing logic ...
```

Update the top-level `main()`:
```rust
#[tokio::main]
async fn main() -> EyreResult<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    let is_json = cli.output == OutputFormat::Json;

    if let Err(e) = run_cli(cli).await {
        if is_json {
            // Build error chain string
            let error_msgs: Vec<String> = e.chain().map(|c| c.to_string()).collect();
            let json_err = serde_json::json!({
                "status": "error",
                "message": error_msgs.join(": ")
            });
            println!("{}", json_err.to_string());
            std::process::exit(1);
        } else {
            // Return error to let color-eyre format it
            return Err(e);
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Update `run_cli` logic returns**

Inside `run_cli`, replace any `Box<dyn std::error::Error>` returns with `EyreResult`.
For the `Commands::Export` block, change:
```rust
// from:
// let stream_result: Result<(), Box<dyn std::error::Error>> = (|| {
// to:
let stream_result: EyreResult<()> = (|| {
```
And replace any `return Err("...".into())` with `return Err(eyre!("..."))`.

- [ ] **Step 4: Verify Compilation**

Run: `cargo check -p eider`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs
git commit -m "refactor: integrate color-eyre and setup json error interception"
```

---

### Task 2: Add Context to Errors in `info` and `extract`

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Update `setup_duckdb` signature**

Update `setup_duckdb` to return `EyreResult`:
```rust
fn setup_duckdb() -> EyreResult<Connection> {
    let conn = Connection::open_in_memory()
        .wrap_err("Failed to open in-memory DuckDB connection")?;

    let ext_path = if cfg!(target_os = "windows") {
        "../target/debug/geozarr.duckdb_extension"
    } else {
        "../target/debug/libgeozarr.duckdb_extension"
    };

    load_geozarr_extension(&conn)
        .wrap_err_with(|| format!("Failed to load geozarr extension from {}", ext_path))?;

    Ok(conn)
}
```

- [ ] **Step 2: Update `load_geozarr_extension` signature**

```rust
fn load_geozarr_extension(conn: &Connection) -> EyreResult<()> {
    let ext_path = if cfg!(target_os = "windows") {
        "../target/debug/geozarr.duckdb_extension"
    } else {
        "../target/debug/libgeozarr.duckdb_extension"
    };
    conn.execute(&format!("LOAD '{}'", ext_path), [])
        .wrap_err_with(|| format!("Failed to load extension at {}", ext_path))?;
    Ok(())
}
```

- [ ] **Step 3: Update `Commands::Info`**

Replace `eprintln!` and `exit`:
```rust
            if let Some(row) = rows.next()? {
                // ... existing printing logic
            } else {
                return Err(eyre!("Failed to read metadata for {}", uri));
            }
```

- [ ] **Step 4: Update `Commands::Extract`**

Update connection and spatial load:
```rust
        Commands::Extract { zarr_uri, vector_path, out } => {
            let config = duckdb::Config::default().allow_unsigned_extensions()
                .wrap_err("Failed to configure unsigned extensions")?;
            let conn = Connection::open_with_flags(&out, config)
                .wrap_err_with(|| format!("Failed to open database at {}", out))?;

            load_geozarr_extension(&conn)?;

            if cli.output != OutputFormat::Json {
                println!("Loading DuckDB spatial extension...");
            }
            conn.execute("INSTALL spatial", []).wrap_err("Failed to install spatial extension")?;
            conn.execute("LOAD spatial", []).wrap_err("Failed to load spatial extension")?;

            if cli.output != OutputFormat::Json {
                println!("Extracting data... This may take a while depending on the bounding box.");
            }

            let query = format!(
                "CREATE OR REPLACE TABLE extracted_data AS \n                 SELECT z.*, v.* EXCLUDE (geom) \n                 FROM read_zarr('{}') z, ST_Read('{}') v \n                 WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat))",
                zarr_uri.replace("'", "''"), vector_path.replace("'", "''")
            );

            conn.execute(&query, []).wrap_err("Spatial extraction query failed")?;

            if cli.output == OutputFormat::Json {
                println!(r#"{{"status": "success", "db": "{}"}}"#, out);
            } else {
                println!("Extraction complete! Data saved to table 'extracted_data' in {}", out);
                println!("Run `eider shell {}` to explore it.", out);
            }
        }
```

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: attach human-readable error context to cli commands using color-eyre"
```

---

### Task 3: Test JSON Error Interception

**Files:**
- Modify: `cli/tests/integration_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_cli_info_invalid_uri_json() {
    let mut cmd = Command::cargo_bin("eider").unwrap();
    cmd.arg("info")
        .arg("s3://invalid-bucket-that-does-not-exist/data.zarr")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#"{"message":"#))
        .stdout(predicate::str::contains(r#""status":"error""#));
}
```

- [ ] **Step 2: Run test to verify it fails**

Wait, it might pass because we already implemented it! But if it fails, fix the logic.
Run: `cargo test -p eider --test integration_test test_cli_info_invalid_uri_json`

- [ ] **Step 3: Update existing test**

Modify the previous `test_cli_info_invalid_uri` to test the non-JSON (table) output instead, verifying it outputs to stderr:
```rust
#[test]
fn test_cli_info_invalid_uri_table() {
    let mut cmd = Command::cargo_bin("eider").unwrap();
    cmd.arg("info")
        .arg("s3://invalid-bucket-that-does-not-exist/data.zarr")
        .arg("--output=table")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to load geozarr extension")
            .or(predicate::str::contains("Failed to read metadata")));
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p eider`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add cli/tests/integration_test.rs
git commit -m "test: add tests verifying JSON and formatted error output"
```
