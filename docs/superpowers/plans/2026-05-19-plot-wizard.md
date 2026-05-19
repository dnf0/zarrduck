# Interactive Plot Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement an interactive wizard for the `zarrduck plot` command that auto-detects the schema and guides the user to the best plot type when invoked without flags.

**Architecture:** We will modify the `Plot` enum variant in `cli/src/main.rs` to make its arguments optional. In `cli/src/plot.rs`, we will introduce a `run_wizard` function that uses the `inquire` crate to prompt for table and variables, applies heuristic logic to recommend plot types, and then delegates back to the existing plot rendering functions.

**Tech Stack:** Rust, DuckDB, `clap`, `inquire`

---

### Task 1: Make CLI Arguments Optional

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Update `Commands::Plot` in `cli/src/main.rs`**

Change the `plot_type` argument to be optional so the command can be invoked simply as `zarrduck plot <db_path>`.

```rust
    /// Plot data from a local DuckDB file
    Plot {
        /// The DuckDB database file
        db_path: String,

        /// Type of plot (hist, heatmap, line). If omitted, launches interactive wizard.
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
    },
```

- [ ] **Step 2: Update `run_cli` to pass the optional `plot_type`**

Modify the match arm in `cli/src/main.rs`:

```rust
// Inside `match cli.command { ... }` in `run_cli`
        Commands::Plot { db_path, plot_type, table, value, group_by } => {
            plot::run_plot(&db_path, plot_type, &table, value.as_deref(), group_by.as_deref())?;
        }
```

- [ ] **Step 3: Verify compilation**

Note: This step will fail initially because we haven't updated `run_plot` signature yet. We will fix it in the next task. For now, just ensure the syntax is correct. (You can skip `cargo check` for this isolated step, or perform Task 2 concurrently). We will combine the compilation check in Task 2.

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat: make plot_type argument optional for wizard fallback"
```

---

### Task 2: Implement Interactive Wizard in `plot.rs`

**Files:**
- Modify: `cli/src/plot.rs`

- [ ] **Step 1: Update `run_plot` signature and add wizard fallback**

```rust
// In cli/src/plot.rs
use inquire::{Select, MultiSelect};

pub fn run_plot(
    db_path: &str,
    plot_type: Option<PlotType>,
    table: &str,
    value_column: Option<&str>,
    group_by: Option<&str>,
) -> Result<()> {
    if !std::path::Path::new(db_path).exists() {
        return Err(eyre!("Database '{}' does not exist.", db_path));
    }

    let conn = Connection::open(db_path)?;

    // If plot_type is provided, run non-interactively
    if let Some(pt) = plot_type {
        let val_col = match value_column {
            Some(v) => v.to_string(),
            None => detect_value_column(&conn, table)?,
        };

        println!("Plotting {} from table {} (Value: {})",
            format!("{:?}", pt).to_lowercase(), table, val_col);

        match pt {
            PlotType::Hist => plot_hist(&conn, table, &val_col, group_by)?,
            PlotType::Heatmap => plot_heatmap(&conn, table, &val_col, group_by)?,
            PlotType::Line => plot_line(&conn, table, &val_col, group_by)?,
        }
        return Ok(());
    }

    // Otherwise, run interactive wizard
    run_wizard(&conn, table)
}
```

- [ ] **Step 2: Implement `run_wizard` function**

```rust
// In cli/src/plot.rs
fn run_wizard(conn: &Connection, default_table: &str) -> Result<()> {
    println!("Launching Interactive Plot Wizard...\n");

    // 1. Select Table
    let mut stmt = conn.prepare("SHOW TABLES")?;
    let mut rows = stmt.query([])?;
    let mut tables = Vec::new();
    while let Some(row) = rows.next()? {
        let table_name: String = row.get(0)?;
        tables.push(table_name);
    }

    if tables.is_empty() {
        return Err(eyre!("No tables found in database."));
    }

    let selected_table = Select::new("Which table would you like to plot?", tables)
        .with_starting_cursor(0)
        .prompt()?;

    // 2. Select Variables
    let mut stmt = conn.prepare(&format!("DESCRIBE \"{}\"", selected_table.replace("\"", "\"\"")))?;
    let mut rows = stmt.query([])?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        let col_name: String = row.get(0)?;
        let col_type: String = row.get(1)?;
        columns.push(format!("{} ({})", col_name, col_type));
    }

    let selected_vars = MultiSelect::new("Select variables to analyze (Space to select, Enter to confirm):", columns)
        .prompt()?;

    if selected_vars.is_empty() {
        println!("No variables selected. Exiting.");
        return Ok(());
    }

    // Extract just the column names
    let var_names: Vec<String> = selected_vars.iter()
        .map(|v| v.split(' ').next().unwrap().to_string())
        .collect();

    // 3. Recommend Plot Type
    let num_vars = var_names.len();
    println!("\nDetected {} variable(s).", num_vars);

    let plot_options = match num_vars {
        1 => vec!["Histogram (Distribution)", "Line Plot (Time Series)"],
        2 => vec!["Scatter Plot (X vs Y)", "Line Plot"],
        _ => vec!["Heatmap (2D Spatial)", "Scatter Plot"],
    };

    let selected_plot_str = Select::new("Choose plot type:", plot_options)
        .with_starting_cursor(0)
        .prompt()?;

    // Map selection to enum
    let plot_type = if selected_plot_str.contains("Histogram") {
        PlotType::Hist
    } else if selected_plot_str.contains("Heatmap") {
        PlotType::Heatmap
    } else if selected_plot_str.contains("Line") {
        PlotType::Line
    } else {
        println!("Plot type '{}' is not fully implemented yet in the renderer. Exiting.", selected_plot_str);
        return Ok(());
    };

    // Determine value column (pick the last selected variable as a heuristic)
    let val_col = var_names.last().unwrap();

    println!("\nExecuting generated command:");
    println!("zarrduck plot <db> --plot-type {} --table {} --value {}\n",
        format!("{:?}", plot_type).to_lowercase(), selected_table, val_col);

    // Delegate to existing rendering functions
    match plot_type {
        PlotType::Hist => plot_hist(conn, &selected_table, val_col, None)?,
        PlotType::Heatmap => plot_heatmap(conn, &selected_table, val_col, None)?,
        PlotType::Line => plot_line(conn, &selected_table, val_col, None)?,
    }

    Ok(())
}
```

- [ ] **Step 3: Fix unit tests in `cli/src/plot.rs`**

Update the unit tests in `plot.rs` to pass `Some(PlotType::...)` where applicable, or just ensure that `plot_hist`, `plot_line`, and `plot_heatmap` calls in the tests are unaffected (they are internal and directly tested, so `run_plot` signature change doesn't break them directly).

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p zarrduck`
Expected: Compilation succeeds.

- [ ] **Step 5: Commit**

```bash
git add cli/src/plot.rs
git commit -m "feat: implement interactive plotting wizard using inquire"
```
