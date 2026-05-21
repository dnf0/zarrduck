# Zarrduck CLI Interactive Prompts & Progress TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate `indicatif` for progress visualization and `inquire` for interactive prompts to improve UX during long-running spatial operations, while preserving headless JSON compatibility for LLM agents.

**Architecture:** We will add the `indicatif` and `inquire` crates. For the `extract` command, we will add an overwrite confirmation using `inquire` before executing the query. To prevent blocking the UI during the DuckDB spatial join, we will run the `indicatif` spinner alongside the DuckDB execution. For the `export` command, we will replace row-count `println!` statements with an `indicatif::ProgressBar`. All TUI logic will be bypassed if `cli.output == OutputFormat::Json`.

**Tech Stack:** Rust, `indicatif`, `inquire`, `tokio`

---

### Task 1: Add Dependencies and Overwrite Protection Prompt

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add dependencies**

Update `cli/Cargo.toml` to add `indicatif` and `inquire`:
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
indicatif = "0.17"
inquire = "0.7"
```

- [ ] **Step 2: Add `inquire` logic to `Commands::Extract`**

In `cli/src/main.rs`, update the `Extract` arm before `let config = duckdb::Config::default().allow_unsigned_extensions()...`:

```rust
        Commands::Extract { zarr_uri, vector_path, out } => {
            // Overwrite protection
            if std::path::Path::new(&out).exists() {
                if cli.output == OutputFormat::Json {
                    return Err(eyre!("Output database '{}' already exists. Aborting to prevent overwrite.", out));
                } else {
                    let ans = inquire::Confirm::new(&format!("File '{}' already exists. Overwrite?", out))
                        .with_default(false)
                        .prompt()
                        .wrap_err("Failed to read user input")?;

                    if !ans {
                        println!("Aborting extraction.");
                        return Ok(());
                    }

                    // User confirmed, so delete the file before opening it with DuckDB
                    std::fs::remove_file(&out).wrap_err_with(|| format!("Failed to delete existing file '{}'", out))?;
                }
            }

            let config = duckdb::Config::default().allow_unsigned_extensions()
// ... existing logic
```

- [ ] **Step 3: Run check to verify it compiles**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs
git commit -m "feat: add overwrite protection prompt using inquire"
```

---

### Task 2: Implement Spinner for Extraction Query

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add `indicatif` logic to `Commands::Extract`**

In `cli/src/main.rs` inside the `Extract` match arm, replace the `println!("Extracting data... This may take a while depending on the bounding box.");` with a spinner:

```rust
            let spinner = if cli.output != OutputFormat::Json {
                let pb = indicatif::ProgressBar::new_spinner();
                pb.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
                        .template("{spinner:.green} {msg}")
                        .unwrap()
                );
                pb.set_message("Performing spatial extraction (this may take a few minutes)...");
                pb.enable_steady_tick(std::time::Duration::from_millis(100));
                Some(pb)
            } else {
                None
            };

            // The magic query: Create a table by joining the GeoZarr pixels that intersect the vector polygons
            let query = format!(
                "CREATE OR REPLACE TABLE extracted_data AS \n                 SELECT z.*, v.* EXCLUDE (geom) \n                 FROM read_zarr('{}') z, ST_Read('{}') v \n                 WHERE ST_Contains(v.geom, ST_Point(z.lon, z.lat))",
                zarr_uri.replace("'", "''"), vector_path.replace("'", "''")
            );

            // Note: Since this is a blocking call, we run it in a blocking task so the tokio runtime can still tick the spinner if needed (though enable_steady_tick actually uses its own background thread).
            conn.execute(&query, []).wrap_err("Spatial extraction query failed")?;

            if let Some(pb) = spinner {
                pb.finish_with_message("Extraction complete!");
            }

            if cli.output == OutputFormat::Json {
```

- [ ] **Step 2: Remove redundant println**

Remove the `if cli.output != OutputFormat::Json { println!("Extraction complete! ..."); }` block since the `spinner.finish_with_message` handles it.

```rust
            if cli.output == OutputFormat::Json {
                println!(r#"{{"status": "success", "db": "{}"}}"#, out);
            } else {
                println!("Run `zarrduck shell {}` to explore the extracted data.", out);
            }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: add indicatif spinner for long-running extraction queries"
```

---

### Task 3: Implement Progress Bar for Export Streaming

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Setup progress bar in `Commands::Export`**

In `cli/src/main.rs` inside the `Export` match arm, after `println!("Pass 2: Streaming data...");` and before `let (tx, mut rx) = ...`:

```rust
            let total_rows_query = format!("SELECT COUNT(*) FROM ({})", query);
            let total_rows: u64 = _conn.query_row(&total_rows_query, [], |row| row.get(0)).unwrap_or(0);

            let progress = if cli.output != OutputFormat::Json && total_rows > 0 {
                let pb = indicatif::ProgressBar::new(total_rows);
                pb.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} rows ({eta})")
                        .unwrap()
                        .progress_chars("#>-")
                );
                Some(pb)
            } else {
                None
            };
```

- [ ] **Step 2: Update stream loop to use progress bar**

Inside the `stream_result` closure, replace the `println!("Streamed {} rows...", row_count);` logic with progress bar updates:

```rust
                    // ... existing eviction check ...

                    row_count += 1;

                    if let Some(ref pb) = progress {
                        if row_count % 10_000 == 0 {
                            pb.set_position(row_count);
                        }
                    }
                }
                Ok(())
            })();
```

- [ ] **Step 3: Finish progress bar**

After `stream_result?;`:
```rust
            if let Some(pb) = progress {
                pb.finish_with_message("Streaming complete");
            } else if cli.output != OutputFormat::Json {
                println!("Finished streaming {} rows.", row_count);
            }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: add indicatif progress bar for streaming export"
```
