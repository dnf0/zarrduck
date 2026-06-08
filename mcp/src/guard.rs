//! Read-only SQL guard for the `run_sql` escape hatch.
//!
//! The MCP session is a single, shared, sandboxed DuckDB connection. The guard
//! keeps `run_sql` from mutating durable state or reaching the filesystem /
//! network: it accepts only a single statement whose first keyword is on an
//! allow-list, restricts `CREATE`/`SET` to temp objects / variables, and
//! rejects any statement containing a denied token (writes, attach, copy-to,
//! exports, extension installs/loads). It is deliberately conservative — when
//! in doubt it rejects.

use color_eyre::eyre::{eyre, Result as EyreResult};

/// Validate that `sql` is a safe, read-only / temp-only single statement.
///
/// Returns `Ok(())` if allowed, otherwise an error describing why it was
/// rejected. This is a best-effort static check, not a parser: it allow-lists
/// the first keyword, constrains `CREATE`/`SET`, and denies a fixed set of
/// mutating/IO tokens.
pub fn ensure_read_only(sql: &str) -> EyreResult<()> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Err(eyre!("empty statement"));
    }
    // A remaining ';' after stripping a single trailing one means multiple
    // statements were supplied.
    if trimmed.contains(';') {
        return Err(eyre!("only a single statement is allowed"));
    }
    let upper = trimmed.to_uppercase();
    let first = upper.split_whitespace().next().unwrap_or("");
    let allowed_first = [
        "SELECT", "WITH", "DESCRIBE", "EXPLAIN", "SHOW", "PRAGMA", "VALUES", "FROM", "TABLE",
        "SET", "CREATE",
    ];
    if !allowed_first.contains(&first) {
        return Err(eyre!(
            "statement type '{first}' is not permitted (read-only session)"
        ));
    }
    // CREATE is only allowed for TEMP tables/views (no durable catalog objects).
    if first == "CREATE"
        && !(upper.contains("TEMP TABLE")
            || upper.contains("TEMPORARY TABLE")
            || upper.contains("TEMP VIEW")
            || upper.contains("TEMPORARY VIEW"))
    {
        return Err(eyre!("CREATE is only allowed for TEMP tables/views"));
    }
    // SET is only allowed for session variables (e.g. SET VARIABLE x = ...),
    // never for engine/config settings that could relax the sandbox.
    if first == "SET" && !upper.starts_with("SET VARIABLE") {
        return Err(eyre!("only SET VARIABLE is allowed"));
    }
    // Denied tokens carry a trailing space so they match the keyword form
    // (e.g. "DROP ") and not an arbitrary identifier substring like
    // "DROPLET" or a column named "update_count".
    for denied in [
        "INSTALL ", "ATTACH ", "COPY ", "EXPORT ", "INSERT ", "UPDATE ", "DELETE ", "DROP ",
        "ALTER ", "LOAD ",
    ] {
        if upper.contains(denied) {
            return Err(eyre!("token '{}' is not permitted", denied.trim()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select() {
        assert!(ensure_read_only("SELECT 1").is_ok());
    }

    #[test]
    fn allows_with_and_temp() {
        assert!(ensure_read_only("WITH a AS (SELECT 1) SELECT * FROM a").is_ok());
        assert!(ensure_read_only("CREATE TEMP TABLE t AS SELECT 1").is_ok());
    }

    #[test]
    fn rejects_writes_and_installs() {
        for s in [
            "COPY t TO 'x.parquet'",
            "INSTALL foo",
            "ATTACH 'x.db'",
            "DELETE FROM t",
            "DROP TABLE t",
            "CREATE TABLE t AS SELECT 1",
            "SELECT 1; DROP TABLE t",
        ] {
            assert!(ensure_read_only(s).is_err(), "should reject: {s}");
        }
    }
}
