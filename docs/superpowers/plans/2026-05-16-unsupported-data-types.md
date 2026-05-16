# Unsupported Data Types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add support for `Bool`, `Int8`, `Int16`, `UInt8`, `UInt16`, `UInt32`, and `UInt64` Zarr arrays.

**Architecture:** Expand the `ChunkBuffer` enum to hold these new primitives. Update `ReadZarrVTab::bind` to map Zarr `DataType` variants to DuckDB `LogicalTypeId` variants. Update `ReadZarrVTab::func` to dispatch these types correctly via `dispatch_yield_loop!`. Implement the `FillValueCmp` trait for all new primitive types.

**Tech Stack:** Rust, `duckdb` (vtab), `zarrs`.

---

### Task 1: Expand ChunkBuffer and Trait Implementations

**Files:**
- Modify: `src/table_function.rs`

- [ ] **Step 1: Write the failing test**
Skip the failing test.

- [ ] **Step 2: Write minimal implementation**
Expand the `ChunkBuffer` enum to include the new types:
```rust
pub enum ChunkBuffer {
    Float32(Vec<f32>),
    Float64(Vec<f64>),
    Int32(Vec<i32>),
    Int64(Vec<i64>),
    String(Vec<String>),
    Bool(Vec<bool>),
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    UInt8(Vec<u8>),
    UInt16(Vec<u16>),
    UInt32(Vec<u32>),
    UInt64(Vec<u64>),
}
```

Add `impl_fill_value_cmp!` for the integer types and implement `FillValueCmp` for `bool`.
```rust
impl_fill_value_cmp!(i32);
impl_fill_value_cmp!(i64);
impl_fill_value_cmp!(f32);
impl_fill_value_cmp!(f64);
// Add new integer implementations:
impl_fill_value_cmp!(i8);
impl_fill_value_cmp!(i16);
impl_fill_value_cmp!(u8);
impl_fill_value_cmp!(u16);
impl_fill_value_cmp!(u32);
impl_fill_value_cmp!(u64);

impl FillValueCmp for bool {
    fn is_fill_value(&self, fill_bytes: &[u8]) -> bool {
        let b = if *self { 1u8 } else { 0u8 };
        [b].as_ref() == fill_bytes
    }
}
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add src/table_function.rs
git commit -m "feat: add chunk buffer variants and fill value traits for all primitives"
```

### Task 2: Logical Type Mapping in Bind

**Files:**
- Modify: `src/table_function.rs`

- [ ] **Step 1: Write the failing test**
Skip the failing test.

- [ ] **Step 2: Write minimal implementation**
Update `ReadZarrVTab::bind` to handle the new types when creating the value column.

Replace this section:
```rust
        // Add the value column based on the array's data type
        let value_type = match array.data_type() {
            DataType::Float32 => LogicalTypeId::Float,
            DataType::Float64 => LogicalTypeId::Double,
            DataType::Int32 => LogicalTypeId::Integer,
            DataType::Int64 => LogicalTypeId::Bigint,
            DataType::String => LogicalTypeId::Varchar,
            _ => return Err(format!("Unsupported data type: {:?}", array.data_type()).into()),
        };
        bind.add_result_column("value", value_type.into());
```
With:
```rust
        // Add the value column based on the array's data type
        let value_type = match array.data_type() {
            DataType::Float32 => LogicalTypeId::Float,
            DataType::Float64 => LogicalTypeId::Double,
            DataType::Int32 => LogicalTypeId::Integer,
            DataType::Int64 => LogicalTypeId::Bigint,
            DataType::String => LogicalTypeId::Varchar,
            DataType::Bool => LogicalTypeId::Boolean,
            DataType::Int8 => LogicalTypeId::Tinyint,
            DataType::Int16 => LogicalTypeId::Smallint,
            DataType::UInt8 => LogicalTypeId::Utinyint,
            DataType::UInt16 => LogicalTypeId::Usmallint,
            DataType::UInt32 => LogicalTypeId::Uinteger,
            DataType::UInt64 => LogicalTypeId::Ubigint,
            _ => return Err(format!("Unsupported data type: {:?}", array.data_type()).into()),
        };
        bind.add_result_column("value", value_type.into());
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add src/table_function.rs
git commit -m "feat: map all zarr primitives to duckdb logical types"
```

### Task 3: Dispatch Macro Updates in Func

**Files:**
- Modify: `src/table_function.rs`

- [ ] **Step 1: Write the failing test**
Skip the failing test.

- [ ] **Step 2: Write minimal implementation**
In `ReadZarrVTab::func`, add the new match arms to the `match bind_data.data_type` block before the `_ => return Err(...)` catch-all.

```rust
            zarrs::array::DataType::Bool => {
                dispatch_yield_loop!(bool, ChunkBuffer::Bool, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::Int8 => {
                dispatch_yield_loop!(i8, ChunkBuffer::Int8, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::Int16 => {
                dispatch_yield_loop!(i16, ChunkBuffer::Int16, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::UInt8 => {
                dispatch_yield_loop!(u8, ChunkBuffer::UInt8, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::UInt16 => {
                dispatch_yield_loop!(u16, ChunkBuffer::UInt16, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::UInt32 => {
                dispatch_yield_loop!(u32, ChunkBuffer::UInt32, output, local_state, &init_data.global_state, bind_data)
            }
            zarrs::array::DataType::UInt64 => {
                dispatch_yield_loop!(u64, ChunkBuffer::UInt64, output, local_state, &init_data.global_state, bind_data)
            }
```

- [ ] **Step 3: Run test to verify it passes**
Run: `cargo test`
Expected: PASS.

- [ ] **Step 4: Commit**
```bash
git add src/table_function.rs
git commit -m "feat: add dispatch logic for all primitive zarr types"
```