# Phase 1: STAC Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a `search` command in the `zarrduck` CLI that queries SpatioTemporal Asset Catalog (STAC) APIs and outputs discoverable Zarr URIs.

**Architecture:** We will add the `stac` crate to interact with the STAC ecosystem. We will add a `Search` variant to the `Commands` enum in `cli/src/main.rs`. We'll write an asynchronous request function that fetches JSON from a given STAC API endpoint, filters the assets for `"application/vnd+zarr"`, and outputs the found URIs. 

**Tech Stack:** Rust, `clap`, `reqwest`, `serde_json`

---

### Task 1: Add Dependencies and Command Structure

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add dependencies**

Update `cli/Cargo.toml` to add `reqwest` for HTTP requests:
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
clap_complete = "4.5"
duckdb = { version = "1.10502.0", features = ["bundled"] }
tokio = { version = "1.0", features = ["full"] }
opendal = { version = "0.48", features = ["services-s3", "services-http"] }
zarrs = { version = "0.16.4", features = ["opendal", "async"] }
serde_json = "1.0"
serde = { version = "1.0", features = ["derive"] }
color-eyre = "0.6"
indicatif = "0.17"
inquire = "0.7"
figment = { version = "0.10", features = ["toml", "env"] }
directories = "5.0"
reqwest = { version = "0.12", features = ["json"] }
```

- [ ] **Step 2: Add `Search` to `Commands` enum**

In `cli/src/main.rs`, update the `Commands` enum:

```rust
    /// Search a STAC API for GeoZarr assets
    Search {
        /// The STAC API URL (e.g., https://planetarycomputer.microsoft.com/api/stac/v1/search)
        #[arg(long)]
        api: String,
        
        /// The collection ID to search (e.g., era5-pds)
        #[arg(long)]
        collection: String,
        
        /// Bounding box (min_lon, min_lat, max_lon, max_lat)
        #[arg(long)]
        bbox: Option<String>,
        
        /// Datetime range (e.g., 2020-01-01T00:00:00Z/2020-12-31T23:59:59Z)
        #[arg(long)]
        datetime: Option<String>,
    },
```

- [ ] **Step 3: Add placeholder logic in `run_cli`**

In `cli/src/main.rs`, inside `run_cli`'s match block:

```rust
        Commands::Search { api, collection, bbox, datetime } => {
            println!("Searching STAC API...");
        }
```

- [ ] **Step 4: Run check to verify it compiles**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs
git commit -m "feat: add reqwest dependency and Search subcommand"
```

---

### Task 2: Implement STAC API Querying

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write STAC fetching logic**

In `cli/src/main.rs` inside the `Search` match arm, replace the placeholder with the actual logic to construct the payload and query the STAC API:

```rust
        Commands::Search { api, collection, bbox, datetime } => {
            let client = reqwest::Client::new();
            
            let mut payload = serde_json::json!({
                "collections": [collection],
                "limit": 10
            });
            
            if let Some(b) = bbox {
                let bbox_arr: Vec<f64> = b.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                if bbox_arr.len() == 4 {
                    payload.as_object_mut().unwrap().insert("bbox".to_string(), serde_json::json!(bbox_arr));
                } else {
                    return Err(eyre!("bbox must be 4 comma-separated numbers (min_lon, min_lat, max_lon, max_lat)"));
                }
            }
            
            if let Some(dt) = datetime {
                payload.as_object_mut().unwrap().insert("datetime".to_string(), serde_json::json!(dt));
            }
            
            if cli.output != OutputFormat::Json {
                println!("Querying STAC API: {}", api);
            }
            
            let res = client.post(&api)
                .json(&payload)
                .send()
                .await
                .wrap_err("Failed to send request to STAC API")?;
                
            if !res.status().is_success() {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                return Err(eyre!("STAC API returned {}: {}", status, text));
            }
            
            let stac_response: serde_json::Value = res.json().await.wrap_err("Failed to parse STAC API response")?;
            
            let mut found_uris = Vec::new();
            
            if let Some(features) = stac_response.get("features").and_then(|f| f.as_array()) {
                for feature in features {
                    if let Some(assets) = feature.get("assets").and_then(|a| a.as_object()) {
                        for (_, asset) in assets {
                            // Planetary computer uses application/vnd+zarr, but sometimes just roles or type "zarr"
                            if let Some(href) = asset.get("href").and_then(|h| h.as_str()) {
                                let is_zarr_type = asset.get("type").and_then(|t| t.as_str()).map_or(false, |t| t.contains("zarr"));
                                let is_zarr_href = href.ends_with(".zarr") || href.contains(".zarr/");
                                
                                if is_zarr_type || is_zarr_href {
                                    found_uris.push(href.to_string());
                                }
                            }
                        }
                    }
                }
            }
            
            if cli.output == OutputFormat::Json {
                let json_out = serde_json::json!({
                    "status": "success",
                    "uris": found_uris
                });
                println!("{}", json_out.to_string());
            } else {
                println!("Found {} Zarr URIs:", found_uris.len());
                for uri in found_uris {
                    println!("- {}", uri);
                }
            }
        }
```

- [ ] **Step 2: Verify Compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: implement STAC API querying and Zarr URI extraction"
```

---

### Task 3: Test STAC Search

**Files:**
- Modify: `cli/tests/integration_test.rs`

- [ ] **Step 1: Add integration test**

In `cli/tests/integration_test.rs`, add a test to ensure the command fails gracefully on invalid APIs:

```rust
#[test]
fn test_cli_search_invalid_api() {
    let mut cmd = Command::cargo_bin("zarrduck").unwrap();
    cmd.arg("search")
        .arg("--api")
        .arg("http://invalid-stac-api-that-does-not-exist.com")
        .arg("--collection")
        .arg("era5")
        .arg("--output=json")
        .assert()
        .failure()
        .stdout(predicate::str::contains(r#""status":"error""#));
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p zarrduck --test integration_test test_cli_search_invalid_api`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add cli/tests/integration_test.rs
git commit -m "test: add integration test for zarrduck search command"
```