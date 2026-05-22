#![allow(unused_macros)]
use color_eyre::eyre::{eyre, Result as EyreResult};
use duckdb::Connection;
use std::sync::Arc;

use geozarr_core::types::ChunkBuffer as ChunkData;

pub struct ChunkUploader {
    pub array: Arc<zarrs::array::Array<dyn zarrs::storage::AsyncWritableStorageTraits>>,
    pub rx: tokio::sync::mpsc::Receiver<(Vec<u64>, ChunkData)>,
    pub progress: Option<indicatif::ProgressBar>,
}

impl ChunkUploader {
    pub fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        let array_clone = self.array.clone();
        tokio::spawn(async move {
            while let Some((chunk_grid, chunk_data)) = self.rx.recv().await {
                let res = match chunk_data {
                    ChunkData::Bool(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Int8(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Int16(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Int32(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Int64(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::UInt8(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::UInt16(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::UInt32(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::UInt64(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Float32(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::Float64(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                    ChunkData::String(data) => {
                        array_clone
                            .async_store_chunk_elements(&chunk_grid, &data)
                            .await
                    }
                };
                if let Err(e) = res {
                    eprintln!("Failed to upload chunk: {}", e);
                    std::process::exit(1);
                }
            }
            if let Some(pb) = self.progress {
                pb.finish_with_message("Streaming complete");
            }
        })
    }
}

struct RowProcessor<'a, 'b> {
    writer: &'a StreamWriter<'b>,
    active_chunks: &'a mut std::collections::BTreeMap<Vec<u64>, ChunkData>,
    tx: tokio::sync::mpsc::Sender<(Vec<u64>, ChunkData)>,
    chunk_len: usize,
    chunk_byte_size: usize,
    max_memory_bytes: usize,
    row_count: &'a mut u64,
    progress: Option<indicatif::ProgressBar>,
}

impl<'a, 'b> RowProcessor<'a, 'b> {
    fn calculate_indices(&self, row: &duckdb::Row) -> EyreResult<(Vec<u64>, u64)> {
        let mut grid_coord = Vec::new();
        for (i, &chunk_dim) in self
            .writer
            .chunk_shape
            .iter()
            .enumerate()
            .take(self.writer.coord_columns.len())
        {
            let val: i64 = row.get(i)?;
            if val < 0 {
                return Err(eyre!(
                    "Coordinates must be positive 0-based integer indices"
                ));
            }
            if (val as u64) >= self.writer.shape[i] {
                return Err(eyre!(
                    "Coordinate index {} exceeds maximum bound of dimension {}",
                    val,
                    self.writer.shape[i]
                ));
            }
            let grid_idx = (val as u64) / chunk_dim;
            grid_coord.push(grid_idx);
        }

        let mut flat_idx = 0;
        let mut stride = 1;
        for i in (0..self.writer.coord_columns.len()).rev() {
            flat_idx += ((row.get::<_, i64>(i)? as u64) % self.writer.chunk_shape[i]) * stride;
            stride *= self.writer.chunk_shape[i];
        }

        Ok((grid_coord, flat_idx))
    }

    #[allow(clippy::cognitive_complexity)]
    fn dispatch_chunk(
        &mut self,
        row: &duckdb::Row,
        grid_coord: Vec<u64>,
        flat_idx: u64,
    ) -> EyreResult<()> {
        let val_col_idx = self.writer.coord_columns.len();

        macro_rules! process_chunk_impl {
            ($rust_type:ty, $enum_variant:path, $default_val:expr, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {{
                let value: Option<$rust_type> = $row.get($val_col_idx)?;
                if let Some(v) = value {
                    let buffer = $active_chunks
                        .entry($grid_coord.clone())
                        .or_insert_with(|| $enum_variant(vec![$default_val; $chunk_len]));
                    if let $enum_variant(b) = buffer {
                        b[$flat_idx as usize] = v;
                    }
                }
            }};
        }

        macro_rules! process_chunk {
            (f32, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                process_chunk_impl!(
                    f32,
                    $enum_variant,
                    f32::NAN,
                    $row,
                    $val_col_idx,
                    $active_chunks,
                    $grid_coord,
                    $chunk_len,
                    $flat_idx
                )
            };
            (f64, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                process_chunk_impl!(
                    f64,
                    $enum_variant,
                    f64::NAN,
                    $row,
                    $val_col_idx,
                    $active_chunks,
                    $grid_coord,
                    $chunk_len,
                    $flat_idx
                )
            };
            ($rust_type:ty, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                process_chunk_impl!(
                    $rust_type,
                    $enum_variant,
                    Default::default(),
                    $row,
                    $val_col_idx,
                    $active_chunks,
                    $grid_coord,
                    $chunk_len,
                    $flat_idx
                )
            };
        }

        geozarr_core::dispatch_zarr_type!(
            self.writer.data_type,
            process_chunk,
            row,
            val_col_idx,
            self.active_chunks,
            grid_coord,
            self.chunk_len,
            flat_idx
        );

        Ok(())
    }

    fn process_row(&mut self, row: &duckdb::Row) -> EyreResult<()> {
        let (grid_coord, flat_idx) = self.calculate_indices(row)?;
        self.dispatch_chunk(row, grid_coord, flat_idx)?;

        while self
            .active_chunks
            .len()
            .saturating_mul(self.chunk_byte_size)
            >= self.max_memory_bytes
        {
            let (oldest_key, evicted_buffer) = self.active_chunks.pop_first().unwrap();
            let tx_clone = self.tx.clone();
            tokio::task::block_in_place(move || {
                tx_clone
                    .blocking_send((oldest_key, evicted_buffer))
                    .map_err(|_| eyre!("Upload worker failed or disconnected"))
            })?;
        }

        *self.row_count += 1;

        if let Some(ref pb) = self.progress {
            if (*self.row_count).is_multiple_of(10_000) {
                pb.set_position(*self.row_count);
            }
        }

        Ok(())
    }
}

pub struct StreamWriter<'a> {
    pub conn: &'a Connection,
    pub query: &'a str,
    pub value_column: &'a str,
    pub coord_columns: Vec<String>,
    pub shape: Vec<u64>,
    pub chunk_shape: Vec<u64>,
    pub data_type: zarrs::array::DataType,
    pub array: Arc<zarrs::array::Array<dyn zarrs::storage::AsyncWritableStorageTraits>>,
    pub is_json: bool,
}

impl<'a> StreamWriter<'a> {
    fn build_stream_query(&self) -> String {
        let mut order_by_parts = Vec::new();
        for (i, c) in self.coord_columns.iter().enumerate() {
            let chunk_dim = self.chunk_shape.get(i).unwrap_or(&1);
            order_by_parts.push(format!(
                "CAST(\"{}\" AS BIGINT) / {}",
                c.replace("\"", "\"\""),
                chunk_dim
            ));
        }
        for c in self.coord_columns.iter() {
            order_by_parts.push(format!("\"{}\"", c.replace("\"", "\"\"")));
        }
        let order_by = order_by_parts.join(", ");
        let coords_str = self
            .coord_columns
            .iter()
            .map(|c| format!("\"{}\"", c.replace("\"", "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "SELECT {}, \"{}\" FROM ({}) ORDER BY {}",
            coords_str,
            self.value_column.replace("\"", "\"\""),
            self.query,
            order_by
        )
    }

    pub async fn stream_data(&self) -> EyreResult<()> {
        let total_rows_query = format!("SELECT COUNT(*) FROM ({})", self.query);
        let total_rows: u64 = self
            .conn
            .query_row(&total_rows_query, [], |row| row.get(0))
            .unwrap_or(0);

        let progress = if !self.is_json && total_rows > 0 {
            let pb = indicatif::ProgressBar::new(total_rows);
            pb.set_style(
                            indicatif::ProgressStyle::default_bar()
                                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} rows ({eta})")
                                .unwrap()
                                .progress_chars("#>-")
                        );
            Some(pb)
        } else {
            None
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<(Vec<u64>, ChunkData)>(16);

        let uploader = ChunkUploader {
            array: self.array.clone(),
            rx,
            progress: progress.clone(),
        };
        let upload_task = uploader.spawn();

        let mut active_chunks: std::collections::BTreeMap<Vec<u64>, ChunkData> =
            std::collections::BTreeMap::new();
        let chunk_len = self
            .chunk_shape
            .iter()
            .try_fold(1u64, |acc, &x| acc.checked_mul(x))
            .ok_or_else(|| eyre!("Chunk volume overflow"))? as usize;

        let bytes_per_element = geozarr_core::types::bytes_per_element(&self.data_type);
        let chunk_byte_size = chunk_len
            .checked_mul(bytes_per_element as usize)
            .ok_or_else(|| eyre!("Chunk byte size overflow"))?;
        let max_memory_bytes = 512 * 1024 * 1024; // 512 MB

        let stream_query = self.build_stream_query();
        let mut stream_stmt = self.conn.prepare(&stream_query)?;

        let mut rows = stream_stmt.query([])?;
        let mut row_count = 0;

        let stream_result: EyreResult<()> = (|| {
            let mut processor = RowProcessor {
                writer: self,
                active_chunks: &mut active_chunks,
                tx: tx.clone(),
                chunk_len,
                chunk_byte_size,
                max_memory_bytes,
                row_count: &mut row_count,
                progress: progress.clone(),
            };
            while let Some(row) = rows.next()? {
                processor.process_row(row)?;
            }
            Ok(())
        })();

        tokio::task::block_in_place(move || {
            for (grid_coord, buffer) in active_chunks.into_iter() {
                let _ = tx.blocking_send((grid_coord, buffer));
            }
        });

        upload_task
            .await
            .map_err(|e| eyre!("Upload task panicked: {}", e))?;

        stream_result?;

        if progress.is_none() && !self.is_json {
            println!("Finished streaming {} rows.", row_count);
        }

        Ok(())
    }
}
