# Zarrduck CLI Configuration Management Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a robust, hierarchical configuration system using `figment` that resolves CLI defaults and S3 credentials from environment variables, local `.zarrduck.toml` files, and global `~/.config/zarrduck/config.toml` files.

**Architecture:** We will create a `config.rs` module that defines the `ZarrduckConfig` and `S3Config` structs, and uses `figment` to layer the configuration sources. We will update the `clap` argument definitions to use `Option<T>` for fields that can fall back to the config. Finally, we will update the DuckDB connection setups to inject AWS credentials using the `CREATE SECRET` command if provided in the config.

**Tech Stack:** Rust, `figment`, `directories`, `serde`

---

### Task 1: Add Dependencies

**Files:**
- Modify: `cli/Cargo.toml`

- [ ] **Step 1: Add dependencies**

Update `cli/Cargo.toml` to add `figment` and `directories`:
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
figment = { version = "0.10", features = ["toml", "env"] }
directories = "5.0"
```

- [ ] **Step 2: Run check to verify it compiles**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add cli/Cargo.toml
git commit -m "chore: add figment and directories for configuration management"
```

---

### Task 2: Create Configuration Module

**Files:**
- Create: `cli/src/config.rs`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Define `ZarrduckConfig` in `config.rs`**

```rust
// In cli/src/config.rs
use serde::Deserialize;
use figment::{Figment, providers::{Format, Toml, Env}};
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Default)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ZarrduckConfig {
    pub output_format: Option<String>,
    pub default_out: Option<String>,
    pub s3: Option<S3Config>,
}

impl ZarrduckConfig {
    pub fn load() -> color_eyre::eyre::Result<Self> {
        let mut figment = Figment::new()
            .merge(Env::prefixed("ZARRDUCK_"));

        // Global config
        if let Some(proj_dirs) = ProjectDirs::from("", "", "zarrduck") {
            let global_config = proj_dirs.config_dir().join("config.toml");
            if global_config.exists() {
                figment = figment.merge(Toml::file(global_config));
            }
        }

        // Local config
        let local_config = std::env::current_dir().unwrap_or_default().join(".zarrduck.toml");
        if local_config.exists() {
            figment = figment.merge(Toml::file(local_config));
        }

        let config: ZarrduckConfig = figment.extract().unwrap_or_else(|_| ZarrduckConfig {
            output_format: None,
            default_out: None,
            s3: None,
        });

        Ok(config)
    }
}
```

- [ ] **Step 2: Declare the module in `main.rs`**

In `cli/src/main.rs`, add the mod declaration near the top:
```rust
mod config;
use config::ZarrduckConfig;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs cli/src/config.rs
git commit -m "feat: implement ZarrduckConfig resolver using figment"
```

---

### Task 3: Integrate Config with CLI Fallbacks

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Update CLI definitions to Option**

In `cli/src/main.rs`, update the `output` arg in the `Cli` struct to be an `Option` and remove the default_value:
```rust
    /// Output format (table or json)
    #[arg(global = true, long)]
    output: Option<OutputFormat>,
```

Update the `out` arg in the `Commands::Extract` subcommand to be an `Option`:
```rust
        /// Output DuckDB database file
        #[arg(long)]
        out: Option<String>,
```

- [ ] **Step 2: Load config and fallback early in `run_cli`**

In `run_cli`, immediately load the config and establish the resolved output format and out paths. Update the function signature and first lines:

```rust
async fn run_cli(mut cli: Cli, config: ZarrduckConfig) -> EyreResult<()> {
    let resolved_output = cli.output.clone()
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

    match cli.command {
```

Update `main()` to load the config and pass it:
```rust
#[tokio::main]
async fn main() -> EyreResult<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let config = ZarrduckConfig::load().unwrap_or_else(|_| ZarrduckConfig { output_format: None, default_out: None, s3: None });
    
    let is_json = cli.output.as_ref().map(|o| *o == OutputFormat::Json)
        .unwrap_or_else(|| config.output_format.as_deref() == Some("json"));
    
    if let Err(e) = run_cli(cli, config).await {
// ... existing error logic
```

- [ ] **Step 3: Update `Commands::Extract` out variable**

In the `Extract` match block, resolve `out`:
```rust
        Commands::Extract { zarr_uri, vector_path, out } => {
            let out_path = out.or(config.default_out)
                .ok_or_else(|| eyre!("Output path not specified. Use --out or set default_out in config."))?;
            
            // Overwrite protection
            if std::path::Path::new(&out_path).exists() {
                if resolved_output == OutputFormat::Json {
                    return Err(eyre!("Output database '{}' already exists. Aborting to prevent overwrite.", out_path));
                } else {
                    let ans = inquire::Confirm::new(&format!("File '{}' already exists. Overwrite?", out_path))
                        .with_default(false)
                        .prompt()
                        .wrap_err("Failed to read user input")?;
                        
                    if !ans {
                        println!("Aborting extraction.");
                        return Ok(());
                    }
                    
                    std::fs::remove_file(&out_path).wrap_err_with(|| format!("Failed to delete existing file '{}'", out_path))?;
                }
            }

            let db_config = duckdb::Config::default().allow_unsigned_extensions()
                .wrap_err("Failed to configure unsigned extensions")?;
            let conn = Connection::open_with_flags(&out_path, db_config)
                .wrap_err_with(|| format!("Failed to open database at {}", out_path))?;

            // Replace all other `out` usages with `out_path` in this block!
```
Make sure you replace all remaining `out` variables inside the `Extract` block with `out_path`. Also fix `if cli.output` to use `if resolved_output` in the rest of `run_cli` if applicable.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: fallback CLI args to config values"
```

---

### Task 4: Inject S3 Credentials into DuckDB

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Update `setup_duckdb` to inject secrets**

Update `setup_duckdb` to take an optional `S3Config` reference and execute `CREATE SECRET`:

```rust
fn setup_duckdb(s3_config: Option<&crate::config::S3Config>) -> EyreResult<Connection> {
    let config = duckdb::Config::default().allow_unsigned_extensions()
        .wrap_err("Failed to configure unsigned extensions")?;
    let conn = Connection::open_in_memory_with_flags(config)
        .wrap_err("Failed to open in-memory DuckDB connection")?;
    
    load_geozarr_extension(&conn)
        .wrap_err("Failed to load geozarr extension")?;
        
    inject_s3_secret(&conn, s3_config)?;
    
    Ok(conn)
}

fn inject_s3_secret(conn: &Connection, s3_config: Option<&crate::config::S3Config>) -> EyreResult<()> {
    if let Some(s3) = s3_config {
        if s3.access_key.is_some() || s3.secret_key.is_some() || s3.region.is_some() || s3.endpoint.is_some() {
            let mut parts = vec!["TYPE S3".to_string()];
            if let Some(ak) = &s3.access_key { parts.push(format!("KEY_ID '{}'", ak.replace("'", "''"))); }
            if let Some(sk) = &s3.secret_key { parts.push(format!("SECRET '{}'", sk.replace("'", "''"))); }
            if let Some(r) = &s3.region { parts.push(format!("REGION '{}'", r.replace("'", "''"))); }
            if let Some(e) = &s3.endpoint { parts.push(format!("ENDPOINT '{}'", e.replace("'", "''"))); }
            
            let query = format!("CREATE SECRET ( {} )", parts.join(", "));
            conn.execute(&query, []).wrap_err("Failed to inject S3 secret into DuckDB")?;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Update usages of `setup_duckdb`**

In `Commands::Info`, update to:
```rust
let conn = setup_duckdb(config.s3.as_ref())?;
```

- [ ] **Step 3: Update `Extract` connection**

In `Commands::Extract`, after `let conn = Connection::open_with_flags...`:
```rust
            load_geozarr_extension(&conn)?;
            inject_s3_secret(&conn, config.s3.as_ref())?;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: inject S3 credentials from config into DuckDB"
```