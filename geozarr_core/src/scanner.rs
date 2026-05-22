use std::sync::Arc;
use zarrs::array::{Array, ElementOwned};
use zarrs::array_subset::ArraySubset;
use zarrs::storage::ReadableStorageTraits;

pub struct GridIterator {
    current: Option<Vec<u64>>,
    bounds_min: Vec<u64>,
    bounds_max: Vec<u64>,
}

impl GridIterator {
    pub fn new(
        bounds_min: &[u64],
        bounds_max: &[u64],
        _shape: &[u64],
        chunk_shape: &[u64],
    ) -> Self {
        let rank = bounds_min.len();
        let mut min = vec![0u64; rank];
        let mut max = vec![0u64; rank];
        for i in 0..rank {
            min[i] = bounds_min[i] / chunk_shape[i];
            max[i] = bounds_max[i] / chunk_shape[i];
        }
        Self {
            current: Some(min.clone()),
            bounds_min: min,
            bounds_max: max,
        }
    }
}

impl Iterator for GridIterator {
    type Item = Vec<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current.take()?;
        let mut next_grid = current.clone();

        let rank = next_grid.len();
        let mut exhausted = true;
        for i in (0..rank).rev() {
            if next_grid[i] < self.bounds_max[i] {
                next_grid[i] += 1;
                exhausted = false;
                break;
            } else {
                next_grid[i] = self.bounds_min[i];
            }
        }

        if !exhausted {
            self.current = Some(next_grid);
        }

        Some(current)
    }
}

#[derive(Default, Clone)]
pub struct SubsetInfo {
    pub global_starts: Vec<u64>,
    pub shape: Vec<u64>,
    pub strides: Vec<u64>,
}

impl SubsetInfo {
    pub fn global_coord(&self, dim: usize, local_pos: u64) -> u64 {
        self.global_starts[dim] + (local_pos / self.strides[dim]) % self.shape[dim]
    }
}

pub struct ChunkReader {
    pub array: Arc<Array<dyn ReadableStorageTraits>>,
    pub is_remote: bool,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
}

impl ChunkReader {
    pub fn new(
        array: Arc<Array<dyn ReadableStorageTraits>>,
        is_remote: bool,
        shape: Vec<u64>,
        chunk_shape: Vec<u64>,
    ) -> Self {
        Self {
            array,
            is_remote,
            shape,
            chunk_shape,
        }
    }

    pub fn read_chunk_subset<T: ElementOwned + Clone>(
        &self,
        grid_coord: &[u64],
        bounds_min: &[u64],
        bounds_max: &[u64],
    ) -> Result<(Vec<T>, SubsetInfo), String> {
        let rank = self.shape.len();
        let mut subset_start = vec![0u64; rank];
        let mut subset_shape = vec![0u64; rank];
        let mut global_starts = vec![0u64; rank];
        let mut strides = vec![1u64; rank];

        for d in 0..rank {
            let chunk_start = grid_coord[d] * self.chunk_shape[d];
            let chunk_end_inc = chunk_start + self.chunk_shape[d] - 1;
            let lo = bounds_min[d].max(chunk_start);
            let hi = bounds_max[d].min(chunk_end_inc);
            subset_start[d] = lo - chunk_start;
            subset_shape[d] = hi - lo + 1;
            global_starts[d] = lo;
        }
        for d in (0..rank - 1).rev() {
            strides[d] = strides[d + 1] * subset_shape[d + 1];
        }

        let elements: Vec<T> = if self.is_remote {
            let full = self
                .array
                .retrieve_chunk_elements::<T>(grid_coord)
                .map_err(|e| format!("zarrs read error: {}", e))?;
            let actual_chunk_shape: Vec<u64> = (0..rank)
                .map(|d| {
                    let chunk_start = grid_coord[d] * self.chunk_shape[d];
                    let chunk_end_inc = chunk_start + self.chunk_shape[d] - 1;
                    let actual_end = self.shape[d].min(chunk_end_inc + 1);
                    actual_end - chunk_start
                })
                .collect();
            let mut chunk_strides = vec![1u64; rank];
            for d in (0..rank - 1).rev() {
                chunk_strides[d] = chunk_strides[d + 1] * actual_chunk_shape[d + 1];
            }
            let total: u64 = subset_shape.iter().product();
            let mut out = Vec::with_capacity(total as usize);
            let mut idx = subset_start.clone();
            for _ in 0..total {
                let flat: u64 = (0..rank).map(|d| idx[d] * chunk_strides[d]).sum();
                out.push(full[flat as usize].clone());
                for d in (0..rank).rev() {
                    idx[d] += 1;
                    if idx[d] < subset_start[d] + subset_shape[d] {
                        break;
                    }
                    idx[d] = subset_start[d];
                }
            }
            out
        } else {
            let chunk_subset =
                ArraySubset::new_with_start_shape(subset_start.clone(), subset_shape.clone())
                    .map_err(|e| format!("Invalid chunk subset: {}", e))?;
            self.array
                .retrieve_chunk_subset_elements::<T>(grid_coord, &chunk_subset)
                .map_err(|e| format!("zarrs read error: {}", e))?
        };

        Ok((
            elements,
            SubsetInfo {
                global_starts,
                shape: subset_shape,
                strides,
            },
        ))
    }
}
