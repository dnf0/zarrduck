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

pub fn run_plot(
    db_path: &str,
    plot_type: PlotType,
    table: &str,
    value_column: Option<&str>,
    _group_by: Option<&str>,
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
