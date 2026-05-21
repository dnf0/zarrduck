# Zarrduck CLI Interactive Prompts & Progress TUI Design

**Date:** 2026-05-18

## 1. Context & Purpose
The `zarrduck` CLI performs heavy spatial operations and network streaming that can take minutes to complete. Currently, the CLI prints static messages and blocks, leaving the user wondering if the process has frozen. Additionally, destructive operations (like overwriting an output file) happen without warning.

The purpose of this sub-project is to introduce `indicatif` for progress visualization and `inquire` for interactive terminal prompts, significantly improving the human UX while strictly preserving headless JSON compatibility for LLM agents.

## 2. Core Architecture & Dependencies

We will add the following crates to `cli/Cargo.toml`:
- `indicatif`: For rendering progress bars and spinners.
- `inquire`: For rendering interactive prompts.

### 2.1 Progress Feedback (`indicatif`)
- **Extraction Command (`zarrduck extract`)**:
  - DuckDB's spatial join executes as a single blocking call.
  - We will spawn a background thread with an `indicatif::ProgressBar::new_spinner()` displaying a message like `"🔄 Performing spatial extraction..."`.
  - The spinner will animate while the query runs and will be cleared/finished with a success message when the query completes.
- **Export Command (`zarrduck export`)**:
  - The export streams data chunk-by-chunk. We will replace the print statements with an `indicatif::ProgressBar` that increments as chunks are processed, giving the user a true percentage-based progress bar.

### 2.2 Interactive Prompts (`inquire`)
- **Extraction Overwrite Protection**:
  - Before executing the `extract` command, we will check if the target `--out` file already exists.
  - If it does, we will use `inquire::Confirm` to prompt: `? File '<name>.duckdb' already exists. Overwrite? [y/N]`.
  - If the user declines, the CLI exits gracefully.

### 2.3 Agent Compatibility Guard
To prevent breaking the "Agentic Spatial Engine" vision, all TUI elements **must** be gated behind the output format check:
- If `cli.output == OutputFormat::Json`:
  - Spinners and progress bars are entirely bypassed.
  - Interactive prompts are suppressed. Instead of prompting for overwrite, the CLI will either default to a safe behavior or abort with a clear JSON error payload indicating manual intervention is required.

## 3. Development Strategy
1. Add `indicatif` and `inquire` to dependencies.
2. Implement the `inquire` overwrite protection in the `extract` command.
3. Implement the `indicatif` spinner during the DuckDB spatial join in the `extract` command.
4. Update the `export` command to use a standard progress bar instead of printing rows.
