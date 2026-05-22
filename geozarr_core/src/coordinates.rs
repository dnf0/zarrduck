use crate::metadata::SpatialTransform;

/// Apply spatial transform to grid coordinates
pub fn apply_transform(transform: &SpatialTransform, dim_index: usize, grid_index: u64) -> f64 {
    let scale = transform.scale.get(dim_index).copied().unwrap_or(1.0);
    let translation = transform.translation.get(dim_index).copied().unwrap_or(0.0);
    translation + (grid_index as f64 * scale)
}

/// Normalize longitude values to -180..180
pub fn normalize_longitude(raw: f64, is_0_360: bool) -> f64 {
    if is_0_360 && raw > 180.0 {
        raw - 360.0
    } else {
        raw
    }
}

/// Convert -180..180 query values back to 0-360 if required
pub fn denormalize_longitude(query_val: f64, is_0_360: bool) -> f64 {
    if is_0_360 && query_val < 0.0 {
        query_val + 360.0
    } else {
        query_val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_transform_coordinate_generation() {
        let transform = SpatialTransform {
            scale: vec![0.1, -0.1],
            translation: vec![-180.0, 90.0],
        };

        assert_eq!(apply_transform(&transform, 0, 5), -180.0 + (5.0 * 0.1));
        assert_eq!(apply_transform(&transform, 1, 10), 90.0 + (10.0 * -0.1));
    }
}
