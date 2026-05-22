import re

with open("cli/src/export.rs", "r") as f:
    content = f.read()

# 1. Replace ChunkData enum
chunk_data_enum = """enum ChunkData {
    Bool(Vec<bool>),
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    UInt8(Vec<u8>),
    UInt16(Vec<u16>),
    UInt32(Vec<u32>),
    UInt64(Vec<u64>),
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    String(Vec<String>),
}"""
content = content.replace(chunk_data_enum, "use geozarr_core::types::ChunkBuffer as ChunkData;")

# 2. Replace match self.data_type
match_block_pattern = r"                let val_col_idx = self\.coord_columns\.len\(\);\n                match self\.data_type \{.*?\n                    _ => return Err\(eyre!\(\"Unsupported DataType\"\)\),\n                \}"

replacement = """                let val_col_idx = self.coord_columns.len();

                macro_rules! process_chunk_impl {
                    ($rust_type:ty, $enum_variant:path, $default_val:expr, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {{
                        let value: Option<$rust_type> = $row.get($val_col_idx)?;
                        let buffer = $active_chunks.entry($grid_coord.clone()).or_insert_with(|| {
                            let mut b = Vec::with_capacity($chunk_len);
                            b.resize($chunk_len, $default_val);
                            $enum_variant(b)
                        });
                        if let Some(v) = value {
                            if let $enum_variant(b) = buffer {
                                b[$flat_idx as usize] = v;
                            }
                        }
                    }};
                }

                macro_rules! process_chunk {
                    (f32, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                        process_chunk_impl!(f32, $enum_variant, f32::NAN, $row, $val_col_idx, $active_chunks, $grid_coord, $chunk_len, $flat_idx)
                    };
                    (f64, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                        process_chunk_impl!(f64, $enum_variant, f64::NAN, $row, $val_col_idx, $active_chunks, $grid_coord, $chunk_len, $flat_idx)
                    };
                    ($rust_type:ty, $enum_variant:path, $row:expr, $val_col_idx:expr, $active_chunks:expr, $grid_coord:expr, $chunk_len:expr, $flat_idx:expr) => {
                        process_chunk_impl!($rust_type, $enum_variant, Default::default(), $row, $val_col_idx, $active_chunks, $grid_coord, $chunk_len, $flat_idx)
                    };
                }

                geozarr_core::dispatch_zarr_type!(
                    self.data_type,
                    process_chunk,
                    row,
                    val_col_idx,
                    active_chunks,
                    grid_coord,
                    chunk_len,
                    flat_idx
                );"""
content = re.sub(match_block_pattern, replacement, content, flags=re.DOTALL)

with open("cli/src/export.rs", "w") as f:
    f.write(content)
