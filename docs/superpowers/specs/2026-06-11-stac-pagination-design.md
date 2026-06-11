# STAC Search API Pagination Design

## Purpose
To enable `eider` to correctly process unbounded or large STAC Search API queries, we need to support pagination. Currently, `eider` only processes the first page of STAC items, which causes benchmarks comparing naive vs pushdown performance to be inaccurate (as naive terminates after 1 page instead of traversing all pages).

## Architecture & Data Flow
When `resolve_sync_store` identifies a remote HTTP path as a STAC collection, it will fetch the initial URL.
Instead of immediately parsing and extracting assets, we will implement a sequential pagination loop:
1. Parse the JSON response.
2. Accumulate the `features` into a running list.
3. Check the `links` array for a `rel` equal to `"next"`.
4. If a `"next"` link exists and provides an `href`, set `fetch_url` to this new `href` and repeat.
5. If no `"next"` link exists, break out of the loop.

After all features have been accumulated across all pages, we construct a consolidated JSON object (or simply pass the accumulated features) to `sorted_features_by_datetime` and the rest of the existing header-fetching logic.

## Components Modified
- `geozarr_core/src/store.rs`: Update the STAC API arm in `resolve_sync_store` to loop and accumulate features. 

## Testing
- Ensure `scripts/bench_stac_pushdown.py` tests pass natively (the naive runner should now show 100 requests).
