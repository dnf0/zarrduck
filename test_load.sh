#!/bin/bash
cd .worktrees/feature-remote-http-extraction
rm -f *.duckdb
./target/debug/zarrduck extract https://mur-sst.s3.us-west-2.amazonaws.com/zarr-v1/analysed_sst scripts/demo_region.geojson --out analysis.duckdb
