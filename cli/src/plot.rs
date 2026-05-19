use color_eyre::eyre::{eyre, Result};
use duckdb::Connection;

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum PlotType {
    Hist,
    Heatmap,
    Line,
}

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
        PlotType::Hist => plot_hist(&conn, table, &val_col, group_by)?,
        PlotType::Heatmap => {}
        PlotType::Line => {}
    }

    Ok(())
}
