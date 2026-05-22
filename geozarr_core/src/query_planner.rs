use std::collections::HashMap;

pub struct QueryConstraints {
    pub bounds: HashMap<String, (Option<f64>, Option<f64>)>,
    pub pins: HashMap<String, u64>,
}

pub struct QueryBounds {
    pub bounds_min: Vec<u64>,
    pub bounds_max: Vec<u64>,
}

pub struct Plan {
    pub bounds: QueryBounds,
}

pub fn translate_filter(
    coords: &[f64],
    operator: &str,
    value: f64,
    current_min: u64,
    current_max: u64,
) -> (u64, u64) {
    if coords.is_empty() {
        return (current_min, current_max);
    }

    let is_ascending = coords.first().unwrap() <= coords.last().unwrap();
    let len = coords.len() as u64;

    let (matched_min, matched_max) = match operator {
        ">" | ">=" => {
            let idx = if is_ascending {
                if operator == ">" {
                    coords.partition_point(|&x| x <= value) as u64
                } else {
                    coords.partition_point(|&x| x < value) as u64
                }
            } else {
                if operator == ">" {
                    coords.partition_point(|&x| x > value) as u64
                } else {
                    coords.partition_point(|&x| x >= value) as u64
                }
            };
            if is_ascending {
                if idx < len {
                    (idx, len - 1)
                } else {
                    return (1, 0); // No matches
                }
            } else {
                if idx > 0 {
                    (0, idx - 1)
                } else {
                    return (1, 0); // No matches
                }
            }
        }
        "<" | "<=" => {
            let idx = if is_ascending {
                if operator == "<" {
                    coords.partition_point(|&x| x < value) as u64
                } else {
                    coords.partition_point(|&x| x <= value) as u64
                }
            } else {
                if operator == "<" {
                    coords.partition_point(|&x| x >= value) as u64
                } else {
                    coords.partition_point(|&x| x > value) as u64
                }
            };
            if is_ascending {
                if idx > 0 {
                    (0, idx - 1)
                } else {
                    return (1, 0); // No matches
                }
            } else {
                if idx < len {
                    (idx, len - 1)
                } else {
                    return (1, 0); // No matches
                }
            }
        }
        "=" => {
            let start = if is_ascending {
                coords.partition_point(|&x| x < value - 1e-8) as u64
            } else {
                coords.partition_point(|&x| x > value + 1e-8) as u64
            };
            let end = if is_ascending {
                coords.partition_point(|&x| x <= value + 1e-8) as u64
            } else {
                coords.partition_point(|&x| x >= value - 1e-8) as u64
            };
            if start < end {
                (start, end - 1)
            } else {
                return (1, 0); // No matches
            }
        }
        _ => return (current_min, current_max),
    };

    (
        std::cmp::max(current_min, matched_min),
        std::cmp::min(current_max, matched_max),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_filter() {
        // Array: [0.0, 10.0, 20.0, 30.0, 40.0, 50.0] (Ascending)
        let coords = vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0];

        // lat = 20.0  => min_idx: 2, max_idx: 2
        let (min, max) = translate_filter(&coords, "=", 20.0, 0, 5);
        assert_eq!((min, max), (2, 2));

        // lat < 25.0 => min_idx: 0, max_idx: 2
        let (min, max) = translate_filter(&coords, "<", 25.0, 0, 5);
        assert_eq!((min, max), (0, 2));

        // lat >= 30.0 => min_idx: 3, max_idx: 5
        let (min, max) = translate_filter(&coords, ">=", 30.0, 0, 5);
        assert_eq!((min, max), (3, 5));

        // Array: [50.0, 40.0, 30.0, 20.0, 10.0, 0.0] (Descending)
        let coords_desc = vec![50.0, 40.0, 30.0, 20.0, 10.0, 0.0];

        // lat = 20.0 => min_idx: 3, max_idx: 3
        let (min, max) = translate_filter(&coords_desc, "=", 20.0, 0, 5);
        assert_eq!((min, max), (3, 3));

        // lat < 25.0 => min_idx: 3, max_idx: 5
        let (min, max) = translate_filter(&coords_desc, "<", 25.0, 0, 5);
        assert_eq!((min, max), (3, 5));

        // lat >= 30.0 => min_idx: 0, max_idx: 2
        let (min, max) = translate_filter(&coords_desc, ">=", 30.0, 0, 5);
        assert_eq!((min, max), (0, 2));

        // lat > 45.0 => min_idx: 0, max_idx: 0
        let (min, max) = translate_filter(&coords_desc, ">", 45.0, 0, 5);
        assert_eq!((min, max), (0, 0));

        // lat <= 10.0 => min_idx: 4, max_idx: 5
        let (min, max) = translate_filter(&coords_desc, "<=", 10.0, 0, 5);
        assert_eq!((min, max), (4, 5));
    }
}
