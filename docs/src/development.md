# Development Guide

Welcome to the Eider contributor documentation! This guide outlines the local development setup, our architectural rules, and the coding standards we follow across both Python and Rust.

## Prerequisites

To build and test the project, you need:
- **Rust:** The latest stable toolchain (`cargo`, `rustc`).
- **Python:** Version 3.12 or newer.
- **uv:** An extremely fast Python package and project manager.

## Building the Project

Eider is structured as a Cargo workspace with three primary crates:

1. `geozarr_core`: The pure Rust domain logic.
2. `extension`: The DuckDB C-API adapter.
3. `cli`: The standalone binary.

To build the entire workspace:

```bash
cargo build --release
```

To build specifically the loadable DuckDB extension file (`.duckdb_extension`):

```bash
cargo duckdb-ext build --release
```

## Dynamic Documentation (Docstrings)

We use Rust's native `cargo doc` to generate dynamic API and internal documentation directly from the source code docstrings. To view the internal developer documentation:

```bash
cargo doc --workspace --no-deps --open
```

This will build the HTML documentation for the `geozarr_core`, `eider`, and `eider_extension` crates and open it automatically in your default web browser.

## Code Style and Guidelines

Our repository employs strict coding standards to ensure that implementations remain clear, maintainable, and highly performant.

### Rust Rules

- **Idioms:** Use standard Rust idioms.
- **Formatting:** All code must be formatted using `cargo fmt`.
- **Linting:** We enforce strict linting. Run `cargo clippy --workspace -- -D warnings` before committing.
- **Error Handling:** Prefer explicit error handling with `Result` and `Option` over panics (`unwrap()` / `expect()`).
- **Complexity:** We heavily discourage "God Methods" or "God Modules". We enforce `clippy::cognitive_complexity` and `clippy::too_many_lines` to keep functions cohesive and readable.
- **Testing:** Write Rust tests using the built-in `#[test]` module and co-locate unit tests within the same file whenever possible. Ensure high coverage of edge cases.
- **Dependencies:** Never leak sink-specific logic (like DuckDB types) into `geozarr_core`. Maintain strict architectural boundaries.

### Python Rules

*(For end-to-end testing scripts and data generation utilities)*

- **Style:** Use Python 3.12 features with explicit typing. Prefer small, single-purpose functions.
- **Environment:** Always use `uv` and virtual environments (`venv`) to create isolated, reproducible environments.
- **Formatting/Linting:** Use `ruff` for formatting and linting.
- **Type Checking:** Use `pyright` for static type checks.
- **Testing:** Write Python tests using the `pytest` framework.

## CI/CD and Verification

Before submitting a Pull Request, ensure that you can run the following verification checklist locally without errors:

1. `cargo check --workspace`
2. `cargo fmt -- --check`
3. `cargo clippy --workspace -- -D warnings`
4. `cargo test --workspace`

We treat failing lint, type, or test checks as blocking. Commits to the main branch must pass all automated CI checks.
