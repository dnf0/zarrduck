# S3 and Cloud Storage Support Design

**Date:** 2026-05-16
**Status:** Approved

## 1. Purpose & Context
The current extension hardcodes `FilesystemStore` which limits reading Zarr datasets to the local disk. Users expect to read large analytical datasets directly from cloud buckets (like `s3://` or `https://`). This design introduces native cloud object storage support.

## 2. Architecture

### 2.1 Dependencies
DuckDB extensions are synchronous, but modern Rust object stores are `async`. To bridge this without spinning up a heavy `tokio` runtime inside DuckDB, we will use the `opendal` crate. OpenDAL natively provides a `BlockingOperator` which abstracts away the async complexity, giving us a clean, synchronous API.
We will add `opendal` (with `services-s3` and `services-http`) and `zarrs_opendal` to `Cargo.toml`.

### 2.2 Dynamic Store Resolution
The `ReadZarrVTab::bind` function receives the target `path` as a string. We will implement a `resolve_store(path: &str) -> Arc<dyn zarrs::storage::ReadableStorageTraits>` helper function to determine the appropriate backend:
- **`s3://` Prefix:** Parse the bucket and path, instantiate an `opendal::services::S3` builder, configure it to load credentials from standard environment variables (e.g. `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`), build a `BlockingOperator`, and wrap it in `zarrs_opendal::OpendalStore`.
- **`http://` or `https://` Prefix:** Instantiate an `opendal::services::Http` builder, build a `BlockingOperator`, and wrap it in `zarrs_opendal::OpendalStore`.
- **Default:** Fallback to the existing `zarrs::storage::store::FilesystemStore`.

### 2.3 Type Erasure
Because our core structs (`ReadZarrBindData`, `ReadZarrInitData`) currently hold an `Arc<FilesystemStore>`, we will type-erase the store by using the `zarrs::storage::ReadableStorageTraits` trait object. This allows the `bind` and `func` logic to remain agnostic to whether the chunks are coming from disk or the network. 

## 3. Security
By using `opendal`, the extension will rely on standard AWS environment variables for S3 credentials. We will document that users need to set these before starting DuckDB.

## 4. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The use of `OpendalStore` (sync wrapper) is perfectly consistent with DuckDB's synchronous execution model.
3. **Scope check:** The scope is tightly bounded to dynamically resolving the `Store` object. The rest of the Zarr querying logic is unaffected.
4. **Ambiguity check:** The credential loading mechanism via env vars is explicitly stated, avoiding confusion over DuckDB secret integration.