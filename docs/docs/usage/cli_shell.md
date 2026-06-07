---
sidebar_position: 9
---

# eider shell

Open an interactive DuckDB shell against a database file, preloading the DuckDB
`spatial` extension and (when the local `duckdb` CLI version matches the bundled
build) the eider extension.

## Synopsis

```
eider shell <db_path>
```

## Arguments

- `db_path` — the DuckDB database file to open.

## Notes

Requires the `duckdb` CLI on your `PATH`. If the local `duckdb` version differs
from the version the eider extension was built against, the shell still opens but
the eider extension is not loaded (a notice is printed).

## Example

```bash
eider shell analysis.duckdb
```
