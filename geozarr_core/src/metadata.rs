use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialTransform {
    pub scale: Vec<f64>,
    pub translation: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoZarrMetadata {
    pub crs: Option<String>,
    #[serde(rename = "spatial_transform")]
    pub transform: Option<SpatialTransform>,
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
}
