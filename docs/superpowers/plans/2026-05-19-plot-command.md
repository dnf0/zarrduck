# Plot Command Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a `zarrduck plot` command to generate inline terminal visualizations (histogram, heatmap, line plot) from local DuckDB files.

**Architecture:** We will add a new `Plot` variant to the `Commands` enum in `cli/src/main.rs`. To keep `main.rs` manageable, the plotting logic will be encapsulated in a new module `cli/src/plot.rs`. DuckDB will handle all aggregations, and Rust will format the results.

**Tech Stack:** Rust, DuckDB, `clap`, `rasciigraph`

---

### Task 1: Update Dependencies and CLI Interface

**Files:**
- Modify: `cli/Cargo.toml`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/plot.rs` (Create)

- [ ] **Step 1: Add `rasciigraph` dependency**

```bash
cargo add rasciigraph --manifest-path cli/Cargo.toml
```

- [ ] **Step 2: Create `cli/src/plot.rs`**

```rust
// cli/src/plot.rs
use color_eyre::eyre::Result;
use duckdb::Connection;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum PlotType {
    Hist,
    Heatmap,
    Line,
}

pub fn run_plot(
    db_path: &str,
    plot_type: PlotType,
    table: &str,
    value_column: Option<&str>,
    group_by: Option<&str>,
) -> Result<()> {
    Ok(())
}
```

- [ ] **Step 3: Register module and update `Commands` in `cli/src/main.rs`**

Modify `cli/src/main.rs` to include the `plot` module and the new `Plot` command:

```rust
// Near the top:
mod config;
mod plot; // ADD THIS

// Inside `enum Commands`:
    /// Plot data from a local DuckDB file
    Plot {
        /// The DuckDB database file
        db_path: String,
        
        /// Type of plot (hist, heatmap, line)
        #[arg(long, value_enum)]
        plot_type: plot::PlotType,
        
        /// The table to query
        #[arg(long, default_value = "extracted_data")]
        table: String,
        
        /// The value column to aggregate (auto-detected if omitted)
        #[arg(long)]
        value: Option<String>,
        
        /// Optional column to group by
        #[arg(long)]
        group_by: Option<String>,
    },
```

- [ ] **Step 4: Route the command in `run_cli` inside `cli/src/main.rs`**

```rust
// Inside `match cli.command { ... }` in `run_cli`
        Commands::Plot { db_path, plot_type, table, value, group_by } => {
            plot::run_plot(&db_path, plot_type, &table, value.as_deref(), group_by.as_deref())?;
        }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 6: Commit**

```bash
git add cli/Cargo.toml cli/src/main.rs cli/src/plot.rs
git commit -m "feat: add plot command CLI interface"
```

---

### Task 2: Implement Helper Functions in `plot.rs`

We need a way to auto-detect the value column if it's not provided, reusing the logic from `main.rs` or writing a simplified version for plotting.

**Files:**
- Modify: `cli/src/plot.rs`

- [ ] **Step 1: Add column detection to `cli/src/plot.rs`**

```rust
// In cli/src/plot.rs
use color_eyre::eyre::{eyre, WrapErr};

fn detect_value_column(conn: &Connection, table: &str) -> Result<String> {
    let mut stmt = conn.prepare(&format!("DESCRIBE \"{}\"", table.replace("\"", "\"\"")))?;
    let mut rows = stmt.query([])?;
    
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_lower = col_name.to_lowercase();
        columns.push((col_name, col_lower));
    }

    let val_col = columns.iter().find(|(_, lower)| {
        !lower.contains("time") && !lower.contains("date") && 
        !lower.contains("lat") && lower != "y" &&
        !lower.contains("lon") && lower != "x" &&
        lower != "geom"
    })
    .map(|(name, _)| name.clone())
    .ok_or_else(|| eyre!("Could not automatically detect a value column"))?;

    Ok(val_col)
}
```

- [ ] **Step 2: Update `run_plot` to connect to DuckDB and resolve the value column**

```rust
// In cli/src/plot.rs
pub fn run_plot(
    db_path: &str,
    plot_type: PlotType,
    table: &str,
    value_column: Option<&str>,
    group_by: Option<&str>,
) -> Result<()> {
    if !std::path::Path::new(db_path).exists() {
        return Err(eyre!("Database '{}' does not exist.", db_path));
    }

    let conn = Connection::open(db_path)?;
    
    let val_col = match value_column {
        Some(v) => v.to_string(),
        None => detect_value_column(&conn, table)?,
    };

    println!("Plotting {} from table {} (Value: {})", 
        format!("{:?}", plot_type).to_lowercase(), table, val_col);

    // Call specific plot functions here later
    match plot_type {
        PlotType::Hist => {}
        PlotType::Heatmap => {}
        PlotType::Line => {}
    }

    Ok(())
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
git add cli/src/plot.rs
git commit -m "feat: add duckdb connection and value column detection to plot"
```

---

### Task 3: Implement Histogram Generation

**Files:**
- Modify: `cli/src/plot.rs`

- [ ] **Step 1: Add `plot_hist` function to `cli/src/plot.rs`**

```rust
// In cli/src/plot.rs
fn plot_hist(conn: &Connection, table: &str, val_col: &str, group_by: Option<&str>) -> Result<()> {
    let group_select = if let Some(g) = group_by {
        format!("\"{}\",", g)
    } else {
        String::new()
    };

    let query = format!(
        "WITH stats AS (
             SELECT min(\"{v}\") as v_min, max(\"{v}\") as v_max FROM \"{t}\"
         ),
         bins AS (
             SELECT 
                 {g}
                 floor((\"{v}\" - v_min) / ((v_max - v_min) / 10.0)) as bin_idx,
                 count(*) as freq
             FROM \"{t}\", stats
             GROUP BY 1, 2
         )
         SELECT {g} bin_idx, freq FROM bins ORDER BY {g} bin_idx",
        v = val_col.replace("\"", "\"\""),
        t = table.replace("\"", "\"\""),
        g = group_select
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    // Since we don't know the exact schema of group_by ahead of time, we'll fetch as strings if present
    println!("Histogram rendering not fully implemented yet. Executed query: {}", query);
    
    // In a real implementation, we'd collect results, find max frequency, and print bars.
    // Let's implement a basic version assuming no group_by for MVP to prove it works.
    let mut max_freq = 0;
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let bin_idx: Option<f64> = if group_by.is_some() { row.get(1)? } else { row.get(0)? };
        let freq: i64 = if group_by.is_some() { row.get(2)? } else { row.get(1)? };
        if let Some(b) = bin_idx {
            max_freq = max_freq.max(freq);
            results.push((b as i32, freq));
        }
    }

    let max_bars = 40;
    for (bin, freq) in results {
        let bars = if max_freq > 0 { (freq as f64 / max_freq as f64 * max_bars as f64) as usize } else { 0 };
        let bar_str = "█".repeat(bars);
        println!("Bin {:2} │ {} ({})", bin, bar_str, freq);
    }

    Ok(())
}
```

- [ ] **Step 2: Hook up `plot_hist` in `run_plot`**

```rust
    match plot_type {
        PlotType::Hist => plot_hist(&conn, table, &val_col, group_by)?,
        PlotType::Heatmap => {}
        PlotType::Line => {}
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
git add cli/src/plot.rs
git commit -m "feat: implement hist plot logic"
```

---

### Task 4: Implement Line Plot Generation

**Files:**
- Modify: `cli/src/plot.rs`

- [ ] **Step 1: Add `plot_line` function to `cli/src/plot.rs` using `rasciigraph`**

```rust
// In cli/src/plot.rs
fn plot_line(conn: &Connection, table: &str, val_col: &str, group_by: Option<&str>) -> Result<()> {
    if group_by.is_some() {
        println!("Warning: group-by is not yet supported for line plots in this MVP. Showing overall line.");
    }

    // We assume there's a time/date column. Let's find it.
    let mut stmt = conn.prepare(&format!("DESCRIBE \"{}\"", table.replace("\"", "\"\"")))?;
    let mut rows = stmt.query([])?;
    let mut time_col = String::from("time"); // Fallback
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_lower = col_name.to_lowercase();
        if col_lower.contains("time") || col_lower.contains("date") {
            time_col = col_name;
            break;
        }
    }

    let query = format!(
        "SELECT \"{v}\" FROM \"{t}\" ORDER BY \"{time}\"",
        v = val_col.replace("\"", "\"\""),
        t = table.replace("\"", "\"\""),
        time = time_col.replace("\"", "\"\"")
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;
    
    let mut data: Vec<f64> = Vec::new();
    while let Some(row) = rows.next()? {
        if let Ok(val) = row.get::<_, f64>(0) {
            data.push(val);
        }
    }

    if data.is_empty() {
        return Err(eyre!("No numeric data found for line plot."));
    }

    let graph = rasciigraph::plot(
        data,
        rasciigraph::Config::default()
            .with_height(15)
            .with_caption(format!("{} over {}", val_col, time_col)),
    );

    println!("\n{}\n", graph);

    Ok(())
}
```

- [ ] **Step 2: Hook up `plot_line` in `run_plot`**

```rust
    match plot_type {
        PlotType::Hist => plot_hist(&conn, table, &val_col, group_by)?,
        PlotType::Heatmap => println!("Heatmap not yet implemented"),
        PlotType::Line => plot_line(&conn, table, &val_col, group_by)?,
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
git add cli/src/plot.rs
git commit -m "feat: implement line plot using rasciigraph"
```

---

### Task 5: Implement Heatmap Plot Generation

**Files:**
- Modify: `cli/src/plot.rs`

- [ ] **Step 1: Add `plot_heatmap` function to `cli/src/plot.rs`**

```rust
// In cli/src/plot.rs
fn plot_heatmap(conn: &Connection, table: &str, val_col: &str, group_by: Option<&str>) -> Result<()> {
    if group_by.is_some() {
         println!("Warning: group-by is ignored for spatial heatmaps.");
    }
    
    // Attempt to find lat/lon columns
    let mut stmt = conn.prepare(&format!("DESCRIBE \"{}\"", table.replace("\"", "\"\"")))?;
    let mut rows = stmt.query([])?;
    let mut lat_col = String::from("lat");
    let mut lon_col = String::from("lon");
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_lower = col_name.to_lowercase();
        if col_lower.contains("lat") || col_lower == "y" { lat_col = col_name.clone(); }
        if col_lower.contains("lon") || col_lower == "x" { lon_col = col_name.clone(); }
    }

    let rows_count = 20;
    let cols_count = 40;

    let query = format!(
        "WITH bounds AS (
            SELECT min(\"{lat}\") as min_lat, max(\"{lat}\") as max_lat,
                   min(\"{lon}\") as min_lon, max(\"{lon}\") as max_lon
            FROM \"{t}\"
        ),
        grid AS (
            SELECT 
                floor((\"{lat}\" - min_lat) / ((max_lat - min_lat) / {rows_count}.0)) as row_idx,
                floor((\"{lon}\" - min_lon) / ((max_lon - min_lon) / {cols_count}.0)) as col_idx,
                avg(\"{v}\") as cell_val
            FROM \"{t}\", bounds
            GROUP BY 1, 2
        )
        SELECT row_idx, col_idx, cell_val FROM grid",
        lat = lat_col.replace("\"", "\"\""),
        lon = lon_col.replace("\"", "\"\""),
        v = val_col.replace("\"", "\"\""),
        t = table.replace("\"", "\"\"")
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;
    
    let mut grid_data = vec![vec![f64::NAN; cols_count]; rows_count];
    let mut global_min = f64::MAX;
    let mut global_max = f64::MIN;

    while let Some(row) = rows.next()? {
        let r: Option<f64> = row.get(0)?;
        let c: Option<f64> = row.get(1)?;
        let v: Option<f64> = row.get(2)?;
        
        if let (Some(r), Some(c), Some(v)) = (r, c, v) {
            let r_idx = r.max(0.0).min((rows_count - 1) as f64) as usize;
            let c_idx = c.max(0.0).min((cols_count - 1) as f64) as usize;
            grid_data[r_idx][c_idx] = v;
            global_min = global_min.min(v);
            global_max = global_max.max(v);
        }
    }

    // ASCII density characters
    let chars = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];
    
    println!("\nHeatmap of {} (Spatial):\n", val_col);
    for r in (0..rows_count).rev() { // Print top-to-bottom
        for c in 0..cols_count {
            let val = grid_data[r][c];
            if val.is_nan() {
                print!("  ");
            } else {
                let normalized = if global_max > global_min {
                    (val - global_min) / (global_max - global_min)
                } else {
                    0.5
                };
                let char_idx = (normalized * (chars.len() as f64 - 1.0)).round() as usize;
                print!("{}{}", chars[char_idx], chars[char_idx]);
            }
        }
        println!();
    }
    println!();
    println!("Legend: Min ({:.2}) -> Max ({:.2}) mapped to density [ {} ]", global_min, global_max, chars.iter().collect::<String>());

    Ok(())
}
```

- [ ] **Step 2: Hook up `plot_heatmap` in `run_plot`**

```rust
    match plot_type {
        PlotType::Hist => plot_hist(&conn, table, &val_col, group_by)?,
        PlotType::Heatmap => plot_heatmap(&conn, table, &val_col, group_by)?,
        PlotType::Line => plot_line(&conn, table, &val_col, group_by)?,
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
git add cli/src/plot.rs
git commit -m "feat: implement heatmap plot using duckdb aggregation"
```
