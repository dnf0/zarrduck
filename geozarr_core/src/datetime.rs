//! RFC3339 datetime → epoch-seconds for STAC `properties.datetime`.

/// Parse an RFC3339 timestamp into seconds since the Unix epoch.
pub fn rfc3339_to_epoch_seconds(s: &str) -> Result<f64, String> {
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .map(|dt| dt.timestamp() as f64)
        .map_err(|e| format!("invalid RFC3339 datetime {s:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_z() {
        assert_eq!(
            rfc3339_to_epoch_seconds("2026-01-01T00:00:00Z").unwrap(),
            1767225600.0
        );
    }
    #[test]
    fn parses_offset() {
        // 2026-01-01T01:00:00+01:00 == 2026-01-01T00:00:00Z
        assert_eq!(
            rfc3339_to_epoch_seconds("2026-01-01T01:00:00+01:00").unwrap(),
            1767225600.0
        );
    }
    #[test]
    fn rejects_garbage() {
        assert!(rfc3339_to_epoch_seconds("not-a-date").is_err());
        assert!(rfc3339_to_epoch_seconds("").is_err());
    }
}
