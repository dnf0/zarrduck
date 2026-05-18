# Zarrduck CLI Tests & Docs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure the `zarrduck` CLI is fully tested via integration tests and comprehensively documented for both human and agent users.

**Architecture:** We will use `assert_cmd` and `predicates` for integration testing. These tools spawn the compiled `zarrduck` binary and assert against its `stdout` and `stderr`, ensuring the CLI contract remains stable. For documentation, we will create a dedicated `cli.md` in the docs site and update the main `README.md` to reflect the new agentic commands.

**Tech Stack:** Rust, `assert_cmd`, `predicates`, Markdown

---

### Task 1: Add Integration Testing Dependencies

**Files:**
- Modify: `cli/Cargo.toml`

- [ ] **Step 1: Write the failing test**
(Skipped for dependency addition)

- [ ] **Step 2: Run test to verify it fails**
(Skipped)

- [ ] **Step 3: Write minimal implementation**

Add the `dev-dependencies` to `cli/Cargo.toml`:

```toml
[dev-dependencies]
assert_cmd = "2.0"
predicates = "3.1"
tempfile = "3.10"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo check -p zarrduck`
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add cli/Cargo.toml
git commit -m "chore: add assert_cmd and predicates for CLI integration testing"
```

---

### Task 2: Implement CLI Integration Tests

**Files:**
- Create: `cli/tests/integration_test.rs`

- [ ] **Step 1: Write the failing test**

```rust
// In cli/tests/integration_test.rs
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("zarrduck").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Agentic Spatial Data Engine"));
}

#[test]
fn test_cli_info_invalid_uri() {
    let mut cmd = Command::cargo_bin("zarrduck").unwrap();
    cmd.arg("info")
        .arg("s3://invalid-bucket-that-does-not-exist/data.zarr")
        .arg("--output=json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read metadata"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zarrduck --test integration_test`
Expected: FAIL (if the binaries aren't built or if the `stderr` string doesn't match exactly). Wait, these should pass immediately because they test the existing implementation! Let's ensure the tests run and pass.

- [ ] **Step 3: Write minimal implementation**
(The tests above *are* the implementation for this task).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p zarrduck --test integration_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add cli/tests/integration_test.rs
git commit -m "test: add integration tests for zarrduck cli commands"
```

---

### Task 3: Overhaul Public Documentation

**Files:**
- Create: `docs/src/cli.md`
- Modify: `docs/src/SUMMARY.md`
- Modify: `README.md`

- [ ] **Step 1: Write `docs/src/cli.md`**

```markdown
# Zarrduck CLI

The `zarrduck` CLI is an Agentic Spatial Data Engine. It allows users and LLM agents to easily discover, extract, and manipulate GeoZarr data directly from the terminal without writing complex spatial SQL.

## Commands

### Discovery: `info`
Quickly inspect the shape, chunking, and Coordinate Reference System (CRS) of a remote Zarr array.

```bash
zarrduck info s3://my-bucket/climate.zarr
```

**Agent Mode:** Use `--output=json` to get a clean, parseable JSON response.
```bash
zarrduck info s3://my-bucket/climate.zarr --output=json
```

### Extraction: `extract`
Perform a Vector-Raster join (zonal extraction). This command downloads only the Zarr chunks that intersect with your vector boundaries, masks the pixels exactly to the polygons, and saves the data to a local DuckDB file.

```bash
zarrduck extract s3://my-bucket/climate.zarr ./my_region.geojson --out analysis.duckdb
```

### Analytics: `shell`
Drop into an interactive DuckDB REPL pre-loaded with the `spatial` and `duckdb_geozarr` extensions.

```bash
zarrduck shell analysis.duckdb
```
```

- [ ] **Step 2: Update `docs/src/SUMMARY.md`**

Add the CLI page to the SUMMARY:

```markdown
# Summary

- [Introduction](./introduction.md)
- [Installation](./installation.md)
- [Usage](./usage.md)
- [Zarrduck CLI](./cli.md)
- [Exporting to Zarr](./exporting.md)
- [How-To Guides](./how_to.md)
- [Performance Metrics](./metrics.md)
- [Architecture](./architecture.md)
```

- [ ] **Step 3: Update `README.md`**

Replace the existing "Quick Start (Writing)" section with the new Zarrduck CLI section:

```markdown
## Zarrduck CLI (Agentic Data Engine)

The companion `zarrduck` CLI allows you to perform complex spatial operations like Vector-Raster joins (Zonal Extraction) from the terminal. It is designed to be fully LLM-agent friendly via the `--output=json` flag.

```bash
# 1. Discover the dataset metadata
zarrduck info s3://my-bucket/climate.zarr --output=json

# 2. Extract raster data strictly within your vector polygons
zarrduck extract s3://my-bucket/climate.zarr ./my_region.geojson --out analysis.duckdb

# 3. Open an interactive spatial SQL shell to analyze the extracted data
zarrduck shell analysis.duckdb
```
```

- [ ] **Step 4: Verify formatting**

Run: `cargo test -p zarrduck` (just to ensure no tests were broken by doc changes)
Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add docs/src/cli.md docs/src/SUMMARY.md README.md
git commit -m "docs: overhaul documentation for the new zarrduck CLI commands"
```
