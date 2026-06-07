use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialTransform {
    pub scale: Vec<f64>,
    pub translation: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialReference {
    pub crs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoZarrMetadata {
    pub crs: Option<String>,
    #[serde(rename = "spatial_reference")]
    pub spatial_reference: Option<SpatialReference>,
    #[serde(rename = "spatial_transform")]
    pub transform: Option<SpatialTransform>,
}

impl GeoZarrMetadata {
    /// CRS resolved from the flat `crs` field, falling back to the nested
    /// `spatial_reference.crs` (the layout used by the GeoZarr spec).
    pub fn resolved_crs(&self) -> Option<String> {
        self.crs
            .clone()
            .or_else(|| self.spatial_reference.as_ref().and_then(|s| s.crs.clone()))
    }
}

pub fn parse_geozarr_metadata(attrs: &Value) -> Option<GeoZarrMetadata> {
    let geozarr_val = attrs.get("geozarr")?;
    serde_json::from_value::<GeoZarrMetadata>(geozarr_val.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_spatial_metadata() {
        let attrs = json!({
            "geozarr": {
                "crs": "EPSG:4326",
                "spatial_transform": {
                    "scale": [0.1, 0.1],
                    "translation": [-180.0, 90.0]
                }
            }
        });

        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.crs, Some("EPSG:4326".to_string()));
        let transform = meta.transform.unwrap();
        assert_eq!(transform.scale, vec![0.1, 0.1]);
        assert_eq!(transform.translation, vec![-180.0, 90.0]);
    }

    #[test]
    fn test_parse_geozarr_missing_crs() {
        let attrs = json!({
            "geozarr": {
                "spatial_transform": {
                    "scale": [0.1, 0.1],
                    "translation": [-180.0, 90.0]
                }
            }
        });

        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.crs, None);
        let transform = meta.transform.unwrap();
        assert_eq!(transform.scale, vec![0.1, 0.1]);
        assert_eq!(transform.translation, vec![-180.0, 90.0]);
    }

    #[test]
    fn test_parse_geozarr_invalid_scale() {
        // scale contains a string instead of f64, should fail to parse GeoZarrMetadata
        // returning None because from_value().ok() fails
        let attrs = json!({
            "geozarr": {
                "crs": "EPSG:4326",
                "spatial_transform": {
                    "scale": [0.1, "invalid"],
                    "translation": [-180.0, 90.0]
                }
            }
        });

        let meta = parse_geozarr_metadata(&attrs);
        assert!(meta.is_none());
    }

    #[test]
    fn test_parse_geozarr_empty() {
        let attrs = json!({
            "geozarr": {}
        });

        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.crs, None);
        assert!(meta.transform.is_none());
    }

    #[test]
    fn test_parse_crs_from_spatial_reference() {
        let attrs = json!({
            "geozarr": {
                "spatial_reference": { "crs": "EPSG:4326" },
                "spatial_transform": {
                    "scale": [1.0, -2.5, 2.5],
                    "translation": [0.0, 90.0, -180.0]
                }
            }
        });
        let meta = parse_geozarr_metadata(&attrs).unwrap();
        assert_eq!(meta.resolved_crs(), Some("EPSG:4326".to_string()));
    }

    #[test]
    fn test_resolved_crs_prefers_flat_then_nested() {
        let flat = json!({ "geozarr": { "crs": "EPSG:3857" } });
        assert_eq!(
            parse_geozarr_metadata(&flat).unwrap().resolved_crs(),
            Some("EPSG:3857".to_string())
        );
        let none = json!({ "geozarr": {} });
        assert_eq!(parse_geozarr_metadata(&none).unwrap().resolved_crs(), None);
    }
}
