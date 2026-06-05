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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_feature_collection() {
        let ds = FeatureCollectionDataset::open("https://example.com/stac").unwrap();
        assert_eq!(ds.url, "https://example.com/stac");
    }
}
