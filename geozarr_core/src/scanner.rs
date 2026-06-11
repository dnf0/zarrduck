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
            let cs = if chunk_shape[i] == 0 { 1 } else { chunk_shape[i] };
            min[i] = bounds_min[i] / cs;
            if bounds_min[i] <= bounds_max[i] {
                max[i] = bounds_max[i] / cs;
            } else {
                max[i] = min[i];
            }
        }
        Self {
            current: Some(min.clone()),
            bounds_min: min,
            bounds_max: max,
        }
    }

    pub fn get_chunk_pos(&self, chunk_idx: u64) -> Vec<u64> {
        let rank = self.bounds_min.len();
        let mut pos = vec![0u64; rank];
        let mut current_idx = chunk_idx;
        
        for i in (0..rank).rev() {
            let mut dim_len = self.bounds_max[i].saturating_sub(self.bounds_min[i]) + 1;
            if self.bounds_max[i] < self.bounds_min[i] {
                dim_len = 1;
            }
            pos[i] = self.bounds_min[i] + (current_idx % dim_len);
            current_idx /= dim_len;
        }
        
        pos
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

#[derive(Default, Clone, Debug)]
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

        let chunk_subset =
            ArraySubset::new_with_start_shape(subset_start.clone(), subset_shape.clone())
                .map_err(|e| format!("Invalid chunk subset: {}", e))?;
        let elements: Vec<T> = self
            .array
            .retrieve_chunk_subset_elements::<T>(grid_coord, &chunk_subset)
            .map_err(|e| format!("zarrs read error: {}", e))?;

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

#[macro_export]
macro_rules! read_chunk_into_buffer_dispatch {
    ($rust_type:ty, $enum_variant:path, $chunk_reader:expr, $grid_pos:expr, $bounds_min:expr, $bounds_max:expr, $buffer:expr) => {{
        let (elements, subset_info) = $chunk_reader.read_chunk_subset::<$rust_type>(
            $grid_pos,
            $bounds_min,
            $bounds_max,
        )?;
        *$buffer = $enum_variant(elements);
        Ok(subset_info)
    }};
}

pub fn read_chunk_into_buffer(
    dataset: &crate::dataset::ZarrDataset,
    grid_pos: &[u64],
    bounds_min: &[u64],
    bounds_max: &[u64],
    buffer: &mut crate::types::ChunkBuffer,
) -> Result<SubsetInfo, String> {
    let chunk_reader = ChunkReader::new(
        dataset.array.clone(),
        dataset.is_remote,
        dataset.shape.clone(),
        dataset.chunk_shape.clone(),
    );

    crate::dispatch_zarr_type!(
        dataset.data_type,
        read_chunk_into_buffer_dispatch,
        &chunk_reader,
        grid_pos,
        bounds_min,
        bounds_max,
        buffer
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use zarrs::array::chunk_grid::{ChunkGrid, RegularChunkGrid};
    use zarrs::array::{ArrayBuilder, DataType, FillValue};
    use zarrs::storage::store::MemoryStore;

    #[test]
    fn test_read_chunk_subset() {
        let store = Arc::new(MemoryStore::new());
        let shape = vec![10, 10];
        let chunk_shape = vec![5, 5];

        let array_write = ArrayBuilder::new(
            shape.clone(),
            DataType::Float32,
            ChunkGrid::new(RegularChunkGrid::new(
                chunk_shape.clone().try_into().unwrap(),
            )),
            FillValue::from(0.0f32),
        )
        .build(store.clone(), "/test")
        .unwrap();

        let mut chunk_data = vec![0.0f32; 25];
        for (i, elem) in chunk_data.iter_mut().enumerate() {
            *elem = i as f32;
        }

        array_write.store_metadata().unwrap();
        array_write
            .store_chunk_elements(&[0, 0], &chunk_data)
            .unwrap();

        let ro_store: Arc<dyn ReadableStorageTraits> = store;
        let array = Array::open(ro_store, "/test").unwrap();

        let reader = ChunkReader::new(Arc::new(array), true, shape, chunk_shape);

        let (elements, info) = reader
            .read_chunk_subset::<f32>(
                &[0, 0],
                &[1, 1], // bounds_min
                &[3, 3], // bounds_max
            )
            .unwrap();

        assert_eq!(info.shape, vec![3, 3]);
        // The chunk data is 5x5:
        // [ 0,  1,  2,  3,  4]
        // [ 5,  6,  7,  8,  9]
        // [10, 11, 12, 13, 14]
        // [15, 16, 17, 18, 19]
        // [20, 21, 22, 23, 24]
        // subset [1..=3, 1..=3] should be:
        // [ 6,  7,  8]
        // [11, 12, 13]
        // [16, 17, 18]
        assert_eq!(
            elements,
            vec![6.0, 7.0, 8.0, 11.0, 12.0, 13.0, 16.0, 17.0, 18.0]
        );
    }

    #[test]
    fn test_grid_iterator_get_chunk_pos() {
        let bounds_min = vec![2, 5, 1];
        let bounds_max = vec![12, 15, 3];
        let shape = vec![20, 20, 20];
        let chunk_shape = vec![4, 4, 2];

        let mut it = GridIterator::new(&bounds_min, &bounds_max, &shape, &chunk_shape);
        
        let mut expected = Vec::new();
        while let Some(pos) = it.next() {
            expected.push(pos);
        }

        let num_chunks = expected.len() as u64;
        let actual: Vec<_> = (0..num_chunks).map(|i| it.get_chunk_pos(i)).collect();

        assert_eq!(actual, expected);
    }
}
