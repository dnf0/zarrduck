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
