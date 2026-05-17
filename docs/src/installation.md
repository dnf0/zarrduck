# Installation

DuckDB GeoZarr is a Cargo Workspace that produces two artifacts: a dynamically loaded DuckDB extension (`.duckdb_extension`) for high-performance reading, and a standalone binary (`geozarr-cli`) for writing data back to Zarr.

## Local Development

If you are developing or building from source, you can build both tools using the standard `cargo` pipeline:

```bash
# Clone the repository
git clone https://github.com/dnf0/duckdb_geozarr.git
cd duckdb_geozarr

# Build the extension and CLI
cargo build --release

# The extension will be located at:
# target/release/libduckdb_geozarr.so (or .dylib / .dll)

# The CLI binary will be located at:
# target/release/geozarr-cli
```

## Loading in DuckDB

Once you have the extension binary, you can load it in DuckDB. 

*Note: Because this is an unsigned community extension, you must explicitly allow unsigned extensions in DuckDB.*

```sql
-- Allow unsigned extensions
SET allow_unsigned_extensions = true;

-- Load the extension (adjust path as needed)
LOAD 'target/release/libduckdb_geozarr.so';
```

If you are using the DuckDB Python client, you can pass this configuration during connection:

```python
import duckdb

conn = duckdb.connect(config={
    'allow_unsigned_extensions': 'true'
})

conn.execute("LOAD 'target/release/libduckdb_geozarr.so'")
```
