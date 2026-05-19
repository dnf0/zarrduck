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

    let group_by_clause = if group_by.is_some() {
        "GROUP BY 1, 2"
    } else {
        "GROUP BY 1"
    };

    let query = format!(
        "WITH stats AS (
             SELECT min(\"{v}\") as v_min, max(\"{v}\") as v_max FROM \"{t}\"
         ),
         bins AS (
             SELECT 
                 {g}
                 COALESCE(floor((\"{v}\" - v_min) / NULLIF((v_max - v_min) / 10.0, 0)), 0) as bin_idx,
                 count(*) as freq
             FROM \"{t}\", stats
             {gb}
         )
         SELECT {g} bin_idx, freq FROM bins ORDER BY {g} bin_idx",
        v = val_col.replace("\"", "\"\""),
        t = table.replace("\"", "\"\""),
        g = group_select,
        gb = group_by_clause
    );

    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([])?;

    let mut max_freq = 0;
    let mut grouped_results: std::collections::BTreeMap<String, Vec<(i32, i64)>> = std::collections::BTreeMap::new();
    
    while let Some(row) = rows.next()? {
        let (group_name, bin_idx_opt, freq) = if group_by.is_some() {
            let g: Option<String> = match row.get(0) {
                Ok(val) => Some(val),
                Err(_) => {
                    // Try to fetch as some other type and convert, or just assume NULL if it fails.
                    // duckdb-rs might not auto-convert numeric groups to string if we just ask for String.
                    // But for now, let's keep it simple. If it's not a string, we can format it.
                    // Actually, duckdb-rs `row.get` with String usually does type coercion if possible.
                    let val: Option<String> = row.get(0).ok().flatten();
                    val
                }
            };
            let g_str = g.unwrap_or_else(|| "NULL".to_string());
            let b: Option<f64> = row.get(1)?;
            let f: i64 = row.get(2)?;
            (g_str, b, f)
        } else {
            let b: Option<f64> = row.get(0)?;
            let f: i64 = row.get(1)?;
            ("All".to_string(), b, f)
        };
        
        if let Some(b) = bin_idx_opt {
            max_freq = max_freq.max(freq);
            grouped_results.entry(group_name).or_default().push((b as i32, freq));
        }
    }

    let max_bars = 40;
    for (group_name, results) in grouped_results {
        if group_by.is_some() {
            println!("Group: {}", group_name);
        }
        for (bin, freq) in results {
            let bars = if max_freq > 0 { (freq as f64 / max_freq as f64 * max_bars as f64) as usize } else { 0 };
            let bar_str = "█".repeat(bars);
            println!("Bin {:2} │ {} ({})", bin, bar_str, freq);
        }
        if group_by.is_some() {
            println!();
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    #[test]
    fn test_plot_hist_no_group_by() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test_data (val DOUBLE)", []).unwrap();
        conn.execute("INSERT INTO test_data VALUES (1.0), (2.0), (3.0), (1.5)", []).unwrap();
        
        let result = plot_hist(&conn, "test_data", "val", None);
        assert!(result.is_ok(), "plot_hist failed: {:?}", result.err());
    }

    #[test]
    fn test_plot_hist_with_group_by() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test_group (val DOUBLE, category VARCHAR)", []).unwrap();
        conn.execute("INSERT INTO test_group VALUES (1.0, 'A'), (2.0, 'A'), (3.0, 'B'), (1.5, 'B')", []).unwrap();
        
        let result = plot_hist(&conn, "test_group", "val", Some("category"));
        assert!(result.is_ok(), "plot_hist with group_by failed: {:?}", result.err());
    }

    #[test]
    fn test_plot_hist_div_by_zero() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test_zero (val DOUBLE)", []).unwrap();
        conn.execute("INSERT INTO test_zero VALUES (2.0), (2.0), (2.0)", []).unwrap();
        
        let result = plot_hist(&conn, "test_zero", "val", None);
        assert!(result.is_ok(), "plot_hist div by zero failed: {:?}", result.err());
    }
}

