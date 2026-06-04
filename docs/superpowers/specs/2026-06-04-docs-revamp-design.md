# Eider Documentation Revamp Design

## Overview
A comprehensive rewrite and modernization of the Eider documentation using Docusaurus. The goal is to create a "state-of-the-art" presentation that serves both end-users (CLI/SQL guides) and engineers (architecture, COG virtualization, and interactive performance benchmarks).

## Framework & Technology
- **Engine**: Docusaurus (React-based, modern MDX support).
- **Aesthetics**: Customized modern theme, dark mode, glassmorphism elements.
- **Interactive Plots**: Plotly.js embedded directly into MDX for dynamic, hoverable performance charts.
- **Diagrams**: Native Mermaid.js integration for architecture and data-flow diagrams.

## Information Architecture (Dual-Track)

### 1. Home / Landing Page
- **Hero Section**: "Zero-Copy Cloud Data in DuckDB."
- **Visuals**: Embedded `demo.gif` showcasing the TUI.
- **Elevator Pitch**: Speed, Scale, Cloud-Native.
- **Ah-ha Snippet**: Side-by-side comparison of Python workflows vs a single Eider SQL query.

### 2. Track A: Using Eider (The User Guide)
Focused on data scientists, analysts, and SQL practitioners.

- **Installation & Setup**
  - Downloading binaries vs building from source.
  - Extension loading (DuckDB CLI, Python, MotherDuck).
  - OpenDAL authentication (`s3://`, `gcs://`).
- **The CLI Tooling & TUI**
  - `eider search`: Multi-level STAC discovery.
  - `eider extract`: Vector-raster intersection and bounding box filtering.
  - `eider resample` & `eider shell`: Analytics and interactive exploration.
- **SQL Query Reference (`read_zarr`)**
  - Basic usage and schema discovery (`read_zarr_metadata`).
  - Spatial pushdown (`lat_min`, `lon_max`).
  - Time filtering and dimension mapping.
- **Exporting Data**
  - Using `COPY ... TO` to write materialized views back to cloud storage.

### 3. Track B: How It Works (The Engineering Deep-Dive)
Focused on technical achievements, performance, and internal logic.

- **System Architecture**
  - **Mermaid Diagram**: The relationship between `geozarr_core`, the DuckDB `extension` C-API, and the `cli`.
  - DuckDB's `bind` and `init` phases for workload allocation.
  - Multi-threaded chunk dispatching to DuckDB workers.
- **Cloud Optimized GeoTIFF (COG) Virtualization**
  - **Mermaid Diagram**: The sequence of the 16KB HTTP byte-range request, IFD parsing, and dynamic `.zarray` JSON synthesis.
  - Explanation of zero-copy virtualization and benchmark results (~2.4ms generation for 10,000 tiles).
- **Spatial Pruning & Data Retrieval**
  - **Mermaid Diagram**: Bounding box translation into chunk grid indices.
  - How this avoids the N+1 network request problem.
- **Interactive Performance Benchmarks (Plotly)**
  - *Head-to-head (Bar)*: Eider vs Xarray vs Zarr-Python (spatial subsetting).
  - *Scaling (Line)*: Throughput vs active DuckDB worker threads.
  - *Network I/O (Waterfall)*: Full file download vs byte-range partial fetching (CMIP6).
  - *Latency (Gauge)*: Coordinate generation baseline (~9.5 µs).

## Deployment
- Integrated into a GitHub Actions workflow to build and deploy to GitHub Pages (`gh-pages`).
