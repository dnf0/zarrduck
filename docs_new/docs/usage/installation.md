# Installation

Eider consists of a loadable DuckDB extension (`eider_extension`) and a CLI tool (`eider`).

## Binary Releases
Download the `.duckdb_extension` binary for your platform from the [Releases page](https://github.com/dnf0/eider/releases).

```sql
-- Allow unsigned extensions
SET allow_unsigned_extensions = true;
-- Load the extension
LOAD '/path/to/eider_extension.duckdb_extension';
```

## Compiling from Source
Requires the Rust toolchain and Cargo.
```bash
git clone https://github.com/dnf0/eider.git
cd eider
cargo build --release
```
The CLI binary will be at `target/release/eider`, and the extension at `target/release/libeider_extension.dylib` (or `.so`).

## Authentication
Eider uses OpenDAL. Configure access by setting standard environment variables:
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`
- `GEOZARR_ALLOW_PATH` (to enable local filesystem access: `export GEOZARR_ALLOW_PATH=/`)
