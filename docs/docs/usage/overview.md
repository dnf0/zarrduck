---
sidebar_position: 1
---

# Eider

Eider is a native DuckDB extension (with a companion CLI) that reads N-dimensional
[Zarr](https://zarr.dev/) / GeoZarr arrays and Cloud-Optimized GeoTIFFs **directly as
flat relational tables** — zero-copy, straight into DuckDB's vectorized engine, with
chunk-level spatial pruning so out-of-range data is never fetched.

## Two ways to use Eider

Eider has two front doors. Pick the one that fits how you work:

| | **DuckDB SQL extension** | **`eider` CLI** |
|---|---|---|
| Best for | Querying arrays as SQL tables | Discovery, extraction, agentic workflows |
| Start here | [SQL quickstart →](./quickstart-sql.md) | [CLI quickstart →](./quickstart-cli.md) |

## Where to go next

- **[Installation](./installation.md)** — get the extension and the CLI.
- **SQL Reference** — every table function and its parameters.
- **CLI Reference** — every `eider` subcommand and flag.
- **Guides** — task-oriented how-tos (extraction, exporting, cloud access).
- **Concepts & Engineering** — architecture, spatial pruning, COG virtualization, benchmarks.
