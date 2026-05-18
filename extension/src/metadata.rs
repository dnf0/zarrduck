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
    pub transform: Option<SpatialTransform>,
}

pub fn parse_geozarr_metadata(attrs: &Value) -> Option<GeoZarrMetadata> {
    let geozarr_val = attrs.get("geozarr")?;
    
    let crs = geozarr_val.get("crs").and_then(|v| v.as_str()).map(|s| s.to_string());
    
    let transform = geozarr_val.get("spatial_transform").and_then(|t| {
        let scale = t.get("scale")?.as_array()?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();
            
        let translation = t.get("translation")?.as_array()?
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();
            
        Some(SpatialTransform { scale, translation })
    });
    
    Some(GeoZarrMetadata { crs, transform })
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
}
