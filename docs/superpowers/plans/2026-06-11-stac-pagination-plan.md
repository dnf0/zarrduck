# STAC Search API Pagination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Implement cursor-based pagination for STAC API requests to accumulate all features across pages.

**Architecture:** We update `resolve_sync_store`'s STAC fallback arm. Instead of fetching the URL once and proceeding, we wrap the STAC request in a loop. We accumulate features into a single array, looking for `links` with `rel="next"` to update the `fetch_url`. When no more pages are found, we inject the consolidated features into the first JSON response body and proceed with normal header-fetching pipeline.

**Tech Stack:** Rust, serde_json, reqwest

---

### Task 1: Add Pagination Loop to `resolve_sync_store`

**Files:**
- Modify: `geozarr_core/src/store.rs`

- [x] **Step 1: Run the failing test**
Run the existing Python test in the benchmark:
Run: `python scripts/bench_stac_pushdown.py test`
Expected: FAIL with `AssertionError: 1 != 100`

- [x] **Step 2: Implement the pagination loop**
In `geozarr_core/src/store.rs`, find `reqwest::blocking::get(&fetch_url)`.
Replace the `if let Ok(resp) = ...` with:
```rust
            let mut fetch_url = if let Some(c) = constraints {
                crate::feature_collection::build_stac_url(path, c)
            } else {
                path.to_string()
            };

            let mut all_features = Vec::new();
            let mut first_json: Option<serde_json::Value> = None;

            loop {
                if let Ok(resp) = reqwest::blocking::get(&fetch_url) {
                    if let Ok(mut json) = resp.json::<serde_json::Value>() {
                        if json.get("stac_version").is_some()
                            && json.get("type").and_then(|t| t.as_str()) == Some("FeatureCollection")
                        {
                            if let Some(features) = json.get_mut("features").and_then(|f| f.as_array_mut()) {
                                all_features.append(features);
                            }

                            if first_json.is_none() {
                                first_json = Some(json.clone());
                            }

                            let mut next_href = None;
                            if let Some(links) = json.get("links").and_then(|l| l.as_array()) {
                                if let Some(next_link) = links.iter().find(|l| l.get("rel").and_then(|r| r.as_str()) == Some("next")) {
                                    if let Some(href) = next_link.get("href").and_then(|h| h.as_str()) {
                                        next_href = Some(href.to_string());
                                    }
                                }
                            }

                            if let Some(href) = next_href {
                                fetch_url = href;
                                continue;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }

            if let Some(mut json) = first_json {
                if !all_features.is_empty() {
                    if let Some(obj) = json.as_object_mut() {
                        obj.insert("features".to_string(), serde_json::Value::Array(all_features));
                    }
                    let sorted = sorted_features_by_datetime(&json)?;
                    // ... the rest of the existing code starting with let asset_names = cog_asset_names(&sorted[0].1)?;
```
Ensure you preserve all the logic that comes after, such as `asset_names = cog_asset_names(...)` and the async thread spawning. You'll need to carefully replace the old block with this new structure.

- [x] **Step 3: Verify it compiles**
Run: `cargo check -p geozarr_core`
Expected: OK

- [x] **Step 4: Verify the test passes**
Build duckdb extension first to make sure Python test uses new code:
Run: `cargo build --release -p eider_extension`
Then run Python test:
Run: `python scripts/bench_stac_pushdown.py test`
Expected: PASS (naive_reqs should now equal 100 because it fetches all pages).

- [x] **Step 5: Commit**
```bash
git add geozarr_core/src/store.rs
git commit -m "feat(geozarr_core): implement stac search api pagination"
```
