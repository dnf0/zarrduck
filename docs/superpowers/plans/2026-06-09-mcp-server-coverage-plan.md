# MCP Server Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Increase test coverage for the `EiderServer` in `mcp/src/server.rs` by directly testing its structs, parameter serialization, and watchdog wrapper logic.

**Architecture:** We will implement unit tests inside a `mod tests` block in `mcp/src/server.rs`. The tests will cover parameter serialization, setup and locking of the `EiderServer` wrapper over an `eider_session`, and tool dispatch via direct async method calls.

**Tech Stack:** Rust, `tokio::test`, `serde_json`, `rmcp`.

---

### Task 1: Add Parameter Serialization Tests

**Files:**
- Modify: `mcp/src/server.rs`

- [ ] **Step 1: Write parameter serialization test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bbox_to_value() {
        let bbox = Bbox {
            lon_min: -10.0,
            lat_min: 20.0,
            lon_max: 10.0,
            lat_max: 40.0,
        };
        let expected = json!({
            "lon_min": -10.0,
            "lat_min": 20.0,
            "lon_max": 10.0,
            "lat_max": 40.0,
        });
        assert_eq!(bbox.to_value(), expected);
    }

    #[test]
    fn test_time_range_to_value() {
        let tr = TimeRange { min: 100.0, max: 200.0 };
        let expected = json!({
            "min": 100.0,
            "max": 200.0,
        });
        assert_eq!(tr.to_value(), expected);
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p eider-mcp -- test_bbox_to_value`
Expected: PASS

Run: `cargo test -p eider-mcp -- test_time_range_to_value`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add mcp/src/server.rs
git commit -m "test: add Bbox and TimeRange serialization tests"
```

### Task 2: Add EiderServer internal `run()` tests

**Files:**
- Modify: `mcp/src/server.rs`

- [ ] **Step 1: Write EiderServer internal run logic tests**

Append to `mod tests`:
```rust
    #[tokio::test]
    async fn test_server_run_success() {
        let conn = eider_session::session().unwrap();
        let server = EiderServer::new(conn);
        let res = server.run(|_conn| {
            Ok(json!({"success": true}))
        }).await.unwrap();

        assert_eq!(res.content[0].text, Some("{\"success\":true}".to_string()));
    }

    #[tokio::test]
    async fn test_server_run_error() {
        let conn = eider_session::session().unwrap();
        let server = EiderServer::new(conn);
        let err_res = server.run(|_conn| {
            Err(color_eyre::eyre::eyre!("duckdb connection error"))
        }).await;

        assert!(err_res.is_err());
        let err = err_res.unwrap_err();
        assert_eq!(err.message, "duckdb connection error");
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p eider-mcp -- test_server_run_`
Expected: PASS (both tests)

- [ ] **Step 3: Commit**

```bash
git add mcp/src/server.rs
git commit -m "test: add EiderServer run method tests"
```

### Task 3: Add EiderServer Tool Dispatch Tests

**Files:**
- Modify: `mcp/src/server.rs`

- [ ] **Step 1: Write EiderServer tool tests**

Append to `mod tests`:
```rust
    #[tokio::test]
    async fn test_run_sql_tool() {
        let conn = eider_session::session().unwrap();
        let server = EiderServer::new(conn);

        let params = RunSqlParams {
            sql: "SELECT 42 as the_answer".to_string(),
            limit: None,
        };

        let res = server.run_sql(params).await.unwrap();
        // Since run_sql returns unstructured CallToolResult via run(), verify it returned successfully
        let text = res.content[0].text.as_ref().unwrap();
        assert!(text.contains("42"));
    }

    #[tokio::test]
    async fn test_describe_table_tool_error_propagation() {
        let conn = eider_session::session().unwrap();
        let server = EiderServer::new(conn);

        let params = DescribeTableParams {
            name: "non_existent_table_99".to_string(),
        };

        // Should return a proper rmcp error rather than panicking
        let res = server.describe_table(params).await;
        assert!(res.is_err());
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p eider-mcp -- test_run_sql_tool`
Expected: PASS

Run: `cargo test -p eider-mcp -- test_describe_table_tool_error_propagation`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add mcp/src/server.rs
git commit -m "test: add tool dispatch and error propagation tests"
```
