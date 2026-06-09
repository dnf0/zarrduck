# Zarr v3 / Remote-Read Fixes — Prioritized Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement each PR task-by-task. Steps use `- [ ]`.

**Context:** The remote partial-read benchmark (PR #139) surfaced three real eider bugs in network reads. This document plans all three, prioritized by impact. **Branch off `main` per PR; never commit to main.** Conventional Commits; `--no-gpg-sign`; trailer `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`. Never `git add -A` (an unrelated dirty `geozarr_core/candidates/mcts_state_scanner.json` lives in the tree — never stage it). Clippy on touched crates (`-p geozarr_core --lib --tests`), `cargo fmt --check`.

## Priority & grouping

| Pri | Bug | Impact | PR |
|---|---|---|---|
| **P0** | End-indexed sharded v3 over HTTP fails (`checksum invalid`) | **Blocks reading default-written sharded v3 over the network** (xarray writes `index_location:"end"` by default) | **PR 1** |
| **P1** | eider fetches whole shards, not inner chunks (~7–8× excess bytes) | Performance on sharded v3 | **PR 1** (same fix) |
| **P2** | Bare COG over HTTP throws `IsADirectory` | Real bug, but has a STAC-URL workaround; COG is secondary | **PR 2** |

**Key insight:** P0 and P1 are the *same* root cause — `AsyncToSyncOpendalStore::get_partial_values_key` (`geozarr_core/src/store.rs:48-88`) fetches the **whole object** (`op.read(&key)`) and only honors `ByteRange::FromStart`, defaulting all else (incl. `FromEnd`, the end-index case) to the whole file. Making it issue real **ranged** reads for **all** `ByteRange` variants fixes correctness (P0) *and* performance (P1) at once. So **PR 1 delivers both P0 and P1**; PR 2 is independent.

---

# PR 1 — Ranged partial reads in the opendal store adapter (P0 + P1)

**Goal:** `AsyncToSyncOpendalStore::get_partial_values_key` issues genuine opendal byte-range GETs resolving every `zarrs::byte_range::ByteRange` variant, so (a) end-indexed shard indices read correctly over HTTP and (b) windowed sharded reads fetch only the needed inner chunks.

**File:** `geozarr_core/src/store.rs` (the `AsyncToSyncOpendalStore` impl, ~lines 48-111). **Tests:** a new `geozarr_core/tests/remote_partial_read.rs` (or extend an existing store test).

**Established facts:**
- `zarrs 0.16.4` `ByteRange` = `FromStart(ByteOffset, Option<ByteLength>) | FromEnd(ByteOffset, Option<ByteLength>)`, with resolver methods that turn a range + total `size` into absolute offsets (`.start(size)`, `.end(size)` / `.length(size)` — verify exact names in `~/.cargo/registry/src/*/zarrs-0.16.4/src/byte_range.rs`).
- `size_key()` (store.rs:90-111) already returns the object size via `op.stat()`.
- opendal ranged read pattern already used in eider: `op.read_with(&key).range(start..end).await` (e.g. store.rs:194 for COG headers).
- zarrs reaches this method via `StoragePartialDecoder::partial_decode` → `get_partial_values_key`; the sharding codec uses it for both the index (FromEnd when `index_location:end`) and inner-chunk reads.

- [ ] **Task 1.1 — Failing correctness test (P0): end-indexed sharded read decodes.**

Build a small **Zarr v3 sharded array with `index_location: "end"`** on a temp dir using `zarrs` (see `~/.cargo/registry/src/*/zarrs-0.16.4/examples/sharded_array_write_read.rs` for the write API; set the sharding codec index location to End). Open it through an **opendal `Fs` operator wrapped in `AsyncToSyncOpendalStore`** (NOT zarrs' native `FilesystemStore` — the bug is in the opendal adapter, so the test must exercise that adapter). Read a sub-region that requires decoding the shard index, and assert it returns the correct values.

```
- [ ] Write tests/remote_partial_read.rs::end_indexed_shard_decodes_via_opendal_adapter
- [ ] Run: cargo test -p geozarr_core --test remote_partial_read end_indexed -- --nocapture
      Expected: FAIL today with "the checksum is invalid" (whole-shard returned for the FromEnd index range)
```

- [ ] **Task 1.2 — Implement ranged reads for all ByteRange variants.**

Rewrite `get_partial_values_key` to resolve each range against the object size and issue a ranged GET. Only `stat` when a range actually needs the size (any `FromEnd`, or `FromStart(_, None)`); otherwise skip the round-trip.

```rust
fn get_partial_values_key(
    &self,
    key: &zarrs::storage::StoreKey,
    byte_ranges: &[zarrs::byte_range::ByteRange],
) -> Result<Option<Vec<bytes::Bytes>>, zarrs::storage::StorageError> {
    use zarrs::byte_range::ByteRange;
    let op = self.operator.clone();
    let key_str = key.as_str().to_string();
    let ranges = byte_ranges.to_vec();
    let needs_size = ranges.iter().any(|r| matches!(
        r, ByteRange::FromEnd(_, _) | ByteRange::FromStart(_, None)));
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Resolve size once iff needed (FromEnd / open-ended ranges).
            let size: u64 = if needs_size {
                match op.stat(&key_str).await {
                    Ok(m) => m.content_length(),
                    Err(e) if e.kind() == opendal::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(zarrs::storage::StorageError::Other(e.to_string())),
                }
            } else { 0 };
            let mut out = Vec::with_capacity(ranges.len());
            for r in &ranges {
                // Use zarrs' resolvers to get absolute [start, end). VERIFY method
                // names against the installed crate (start(size)/end(size)).
                let start = r.start(size);
                let end = r.end(size); // exclusive
                match op.read_with(&key_str).range(start..end).await {
                    Ok(buf) => out.push(bytes::Bytes::from(buf.to_vec())),
                    Err(e) if e.kind() == opendal::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(zarrs::storage::StorageError::Other(e.to_string())),
                }
            }
            Ok(Some(out))
        })
    }).join().unwrap()
}
```
> VERIFY: the exact `ByteRange` resolver method names/signatures in zarrs 0.16.4 (`start(size)`, `end(size)` — or `to_range(size) -> Range<u64>`). If a resolver isn't public, compute manually: `FromStart(o,Some(l)) → o..o+l`; `FromStart(o,None) → o..size`; `FromEnd(o,Some(l)) → (size-o-l)..(size-o)`; `FromEnd(o,None) → 0..(size-o)`. Match the crate's exact semantics (mind whether `FromEnd` offset is measured from the end).

```
- [ ] Run Task 1.1 test → PASS (checksum error gone).
```

- [ ] **Task 1.3 — Failing perf test (P1): windowed sharded read fetches < whole shard.**

Wrap the opendal operator (or the store) so total bytes returned by `get_partial_values_key` are counted (e.g. an opendal layer, or sum `buf.len()` via a test-only wrapper store). Build a sharded array where a shard holds many inner chunks; read a window touching a small subset; assert bytes returned ≪ one whole shard.

```
- [ ] Write tests/remote_partial_read.rs::windowed_sharded_read_is_partial
- [ ] Expected after Task 1.2: bytes fetched ≈ the touched inner chunks (+ index), not the whole shard.
```
(If, after Task 1.2, zarrs still requests whole-shard ranges — i.e. the scanner retrieves at outer-shard granularity rather than letting zarrs' partial decoder range inner chunks — investigate `geozarr_core/src/scanner.rs:126` `retrieve_chunk_subset_elements(grid_coord, chunk_subset)`: confirm `grid_coord` is the shard and `chunk_subset` the window, so zarrs' `StoragePartialDecoder` issues inner-chunk ranges. If the scanner bypasses partial decoding, add using `array.partial_decoder(&shard).partial_decode(&inner_subsets)` per the zarrs example. Report if this larger change is needed.)

- [ ] **Task 1.4 — Guard the non-regressing paths + verify.**
Confirm start-indexed shards, non-sharded v2/v3, and local-FS reads still pass. `cargo test -p geozarr_core`; `cargo test -p eider_extension`; `cargo fmt --check`; `cargo clippy -p geozarr_core --lib --tests -- -D warnings`.

- [x] **Task 1.5 — End-to-end re-verify with the benchmark.** Rebuild the extension; re-run `scripts/bench_remote_partialread.py --formats zarr_v3_sharded --windows 0.01 0.1 --shape 4000 4000` and confirm (a) the gate passes for an **end-indexed** sharded store (regenerate the fixture with default `index_location` / drop the `start` workaround) and (b) eider's sharded bytes drop toward the chunk-aware baseline. Update `docs/docs/engineering/benchmarks.mdx` sharded numbers + remove the "end-indexed fails / whole-shard" caveats now fixed.

- [ ] **Task 1.6 — Commit.** `fix(store): ranged opendal partial reads (end-indexed shards + partial-shard reads over HTTP)`.

---

# PR 2 — Split endpoint/key for HTTP operators (P2: bare COG over HTTP)

**Goal:** `read_geo('http://host/path/x.tif')` (and STAC FeatureCollection COG assets) read correctly over HTTP, instead of `IsADirectory`.

**File:** `geozarr_core/src/store.rs`. **Tests:** `geozarr_core/tests/http_cog.rs` (local HTTP server fixture).

**Root cause:** three branches build the opendal HTTP operator as `Http::default().endpoint(<full_url>)` with read key `""` (opendal then reads the root → `IsADirectory`). The single-Feature STAC-item branch (~lines 689-703) does it correctly: split `endpoint = scheme://host[:port]` and `key = url.path()`.

**Broken call sites (verify current line numbers):**
- ~line 741 — bare single-remote-COG (`endpoint(path)`, key `""`).
- ~line 427 — STAC FeatureCollection per-asset (`endpoint(&href)`, key `""`).
- ~line 579 — FeatureCollection variant (same mistake).
- ~line 991 — `list_arrays` (`endpoint(uri)`); fix for completeness (reads `.zgroup` etc. against a mis-rooted endpoint).

- [ ] **Task 2.1 — Extract a DRY helper + unit test.**
```rust
/// Split an http(s) URL into an opendal Http `endpoint` (scheme://host[:port])
/// and the read `key` (path without leading '/'). Mirrors the working
/// single-Feature STAC-item branch.
fn split_http_endpoint_key(url_str: &str) -> Result<(String, String), String> {
    let url = reqwest::Url::parse(url_str).map_err(|e| e.to_string())?;
    let port = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
    let host = url.host_str().ok_or("missing host")?;
    let endpoint = format!("{}://{}{}", url.scheme(), host, port);
    let key = url.path().trim_start_matches('/').to_string();
    Ok((endpoint, key))
}
```
Unit test: `http://h:8080/a/b/grid.tif` → `("http://h:8080", "a/b/grid.tif")`; `https://h/x.tif` → `("https://h", "x.tif")`. Fail→pass.

- [ ] **Task 2.2 — Failing integration test: bare COG over local HTTP.**
Add a local HTTP server fixture (a tiny static-file server over a committed/generated small COG; `wiremock` or a `std`/`tiny_http` server). `resolve_sync_store("http://127.0.0.1:PORT/grid.tif")` then a `read_geo`-style read returns the expected values. Expected: FAIL today (`IsADirectory`).

- [ ] **Task 2.3 — Apply the helper at all four sites.** Replace each `endpoint(full_url)` + key `""` with `let (endpoint, key) = split_http_endpoint_key(url)?;` then `Http::default().endpoint(&endpoint)` and use `key` as the read key (the COG header read `read_with(&key).range(0..N)`, and the `VirtualCogStore` read key = `key`). Keep the S3 branch logic (it already derives bucket/path). Run Task 2.2 → PASS.

- [ ] **Task 2.4 — Verify + commit.** `cargo test -p geozarr_core` (+ extension); fmt; clippy. Also confirm Zarr-over-HTTP still works (it used non-empty keys; the helper makes its endpoint construction consistent too — re-run a Zarr http read). Commit `fix(store): split endpoint/key for HTTP operators (bare COG over HTTP)`.

---

## Self-review
- **P0+P1 = one fix** (the partial-read adapter) — verified by a correctness test (end-indexed decode) AND a perf test (partial-shard bytes); Task 1.3 flags if a scanner-level change is also needed.
- **P2** mirrors an existing-correct branch via a DRY helper across all four call sites; covered by a local-HTTP-server test.
- Tests exercise the **opendal adapter specifically** (not zarrs' native FilesystemStore), since that's where the bugs live.
- Blast radius is low and called out per PR; non-sharded/local/start-indexed paths are guarded.
- Each PR rebuilds the extension and (PR 1) re-runs the benchmark to confirm the documented numbers improve and the caveats can be dropped.
