pub struct FeatureCollectionDataset {
    pub url: String,
    pub asset_name: String,
}

impl FeatureCollectionDataset {
    pub fn open(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Simple extraction: assume URL might have asset name as fragment or parameter
        // For minimal scaffolding, just store the url.
        Ok(Self {
            url: url.to_string(),
            asset_name: "swir22".to_string(), // hardcoded for scaffolding
        })
    }
}

pub fn build_stac_url(
    base_url: &str,
    constraints: &crate::query_planner::QueryConstraints,
) -> String {
    let mut url = base_url.to_string();

    let lat_bounds = constraints
        .bounds
        .get("lat")
        .copied()
        .unwrap_or((None, None));
    let lon_bounds = constraints
        .bounds
        .get("lon")
        .copied()
        .unwrap_or((None, None));

    if let (Some(lon_min), Some(lat_min), Some(lon_max), Some(lat_max)) =
        (lon_bounds.0, lat_bounds.0, lon_bounds.1, lat_bounds.1)
    {
        let separator = if url.contains('?') { "&" } else { "?" };
        url = format!(
            "{}{separator}bbox={},{},{},{}",
            url, lon_min, lat_min, lon_max, lat_max
        );
    }

    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_feature_collection() {
        let ds = FeatureCollectionDataset::open("https://example.com/stac").unwrap();
        assert_eq!(ds.url, "https://example.com/stac");
    }

    #[test]
    fn test_stac_filter_pushdown() {
        let mut bounds = std::collections::HashMap::new();
        bounds.insert("lat".to_string(), (Some(40.0), Some(45.0)));
        bounds.insert("lon".to_string(), (Some(-10.0), Some(10.0)));
        let constraints = crate::query_planner::QueryConstraints {
            bounds,
            pins: std::collections::HashMap::new(),
        };

        let url =
            crate::feature_collection::build_stac_url("https://example.com/search", &constraints);
        assert!(url.contains("bbox=-10,40,10,45"));
    }
}
