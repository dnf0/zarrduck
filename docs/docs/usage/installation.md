# Installation

Eider ships as two pieces that work together: a loadable **DuckDB extension** (queried from SQL) and the **`eider` CLI**.

## DuckDB extension

### From a release

Download the `eider-<platform>.duckdb_extension` for your platform from the
[Releases page](https://github.com/dnf0/eider/releases) and rename it to
`eider.duckdb_extension` — DuckDB derives the load entry point from the filename.

Launch DuckDB allowing unsigned extensions. The flag must be set **at startup**
(`SET allow_unsigned_extensions` cannot be changed at runtime), and `LOAD`
requires an **absolute** path:

```bash
duckdb -unsigned
```

```sql
LOAD '/absolute/path/to/eider.duckdb_extension';
```

### From source

Requires the Rust toolchain and `cargo-duckdb-ext-tools`
(`cargo install cargo-duckdb-ext-tools`):

```bash
git clone https://github.com/dnf0/eider.git
cd eider
cargo duckdb-ext build -o target/debug/eider.duckdb_extension \
  -d v1.5.2 -- --no-default-features --features loadable-extension
```

## CLI

Download the `eider` binary from the [Releases page](https://github.com/dnf0/eider/releases),
or build from source:

```bash
cargo build --release -p eider   # binary at target/release/eider
```

## Authentication & access

Eider streams data through [Apache OpenDAL](https://opendal.apache.org/).
Configure access with standard environment variables:

- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` — for `s3://`
- `GEOZARR_ALLOW_PATH` — permit local filesystem reads, e.g. `export GEOZARR_ALLOW_PATH=/`
