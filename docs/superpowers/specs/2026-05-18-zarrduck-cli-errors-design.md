# Zarrduck CLI Error Diagnostics Design

**Date:** 2026-05-18

## 1. Context & Purpose
As the `zarrduck` CLI grows in complexity (handling S3 fetching, DuckDB spatial joins, and metadata parsing), failures become inevitable. Currently, errors are either returned as raw, unformatted Rust strings or printed via basic `eprintln!`. This provides a poor User Experience (UX).

The purpose of this sub-project is to integrate the `color-eyre` crate to provide beautiful, annotated, and colorful error diagnostics and panic backtraces for human users, while preserving strict JSON error formatting for LLM agents.

## 2. Core Architecture

We will adopt **`color-eyre`** as the global error handling framework for the CLI.

### 2.1 Initialization & Wrapping
- The `color-eyre` panic and error reporting hooks will be installed at the very top of `main()`.
- The `main()` function will return `color_eyre::Result<()>`.
- The existing `Box<dyn std::error::Error>` returns across the CLI will be refactored to use `color_eyre::Result`.

### 2.2 Error Context
Instead of simply bubbling up opaque library errors (like "DuckDB IO Error"), we will utilize the `eyre::WrapErr` trait to attach human-readable context.
- Example: `let conn = Connection::open(&out).wrap_err_with(|| format!("Failed to open local DuckDB database at {}", out))?;`
- This ensures the user sees exactly *what* the CLI was trying to do when the underlying library failed.

## 3. Agent-First Constraints
The CLI supports an `--output=json` mode designed for LLM agents. `color-eyre` outputs highly formatted ANSI-colored ASCII art, which fundamentally breaks JSON parsing.

### 3.1 JSON Error Interception
To ensure compatibility, we will wrap the command execution logic in a helper function (`run_cli() -> color_eyre::Result<()>`).
Inside `main()`, we will match the result of `run_cli()`:
- If `Ok(())`, exit normally.
- If `Err(e)`:
  - If `--output=json` was requested, we will manually serialize the error chain into a JSON payload: `{"status": "error", "message": "<error_chain>"}` and print it to stdout, exiting with code 1.
  - If standard table output was requested, we will return `Err(e)`, allowing `color-eyre` to print its beautiful terminal diagnostics.

## 4. Development Strategy
1. Add `color-eyre` to `cli/Cargo.toml`.
2. Refactor `cli/src/main.rs` to extract the core logic into `run_cli()`.
3. Implement the JSON error interception in `main()`.
4. Replace raw `eprintln!` and `std::process::exit(1)` calls with `eyre` context propagation.
