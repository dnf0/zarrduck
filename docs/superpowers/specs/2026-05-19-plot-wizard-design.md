# Design Specification: Interactive Plotting Wizard

## Overview
An interactive wizard mode for the `zarrduck plot` command. When a user runs the command without explicit flags, they will be dropped into an interactive session (powered by the `inquire` crate) to select their data and determine the best visualization strategy based on their selections.

## User Experience

1. **Invocation:** The user runs `zarrduck plot <db_path>`. The CLI detects that no strict `--plot-type` or `--var` flags were passed and initiates the wizard.
2. **Table Selection:** The CLI queries the DuckDB file for a list of tables and presents them in a selectable list.
3. **Variable Selection:** The CLI queries the selected table's schema and presents a multi-select checklist of all columns. The user selects the variables they wish to plot.
4. **Plot Recommendation:** Based on the number of variables selected, the CLI recommends plot types:
    *   **1 Variable:** Histogram or Line Plot (auto-detecting time).
    *   **2 Variables:** Scatter Plot or Line Plot.
    *   **3 Variables:** Heatmap.
5. **Execution:** Once the user selects the desired plot type, the existing rendering engine takes over and prints the plot to the terminal.

## Architecture

*   **Wizard Module:** A new `wizard` function in `cli/src/plot.rs` will handle the interactive logic.
*   **Dependency:** The `inquire` crate will be used for standardizing the UI components (Select, MultiSelect).
*   **Non-Interactive Fallback:** The existing flag-driven logic will remain intact. If flags are provided (e.g., in a CI environment or script), the wizard is bypassed.

## Components to Update
*   **`cli/src/plot.rs`**: Add the `wizard` logic. Refactor `run_plot` to check for missing arguments and branch into the wizard.
*   **`cli/src/main.rs`**: Make the arguments on the `Plot` enum variant optional to allow invocation with just the database path.