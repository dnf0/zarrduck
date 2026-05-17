# Unsupported Data Types Design

**Date:** 2026-05-16
**Status:** Approved

## 1. Purpose & Context
The current extension only supports reading `f32`, `f64`, `i32`, `i64`, and `String` arrays. If a user points the extension at a valid Zarr array using booleans, unsigned integers, or smaller signed integers, DuckDB crashes or returns an unsupported type error. We need to expand our type mappings to natively support all common Zarr primitives.

## 2. Architecture

### 2.1 Logical Type Mapping
We will expand `ReadZarrVTab::bind` to accurately map Zarr data types to DuckDB `LogicalTypeId`s for the `value` column:
- `DataType::Bool` -> `LogicalTypeId::Boolean`
- `DataType::Int8` -> `LogicalTypeId::Tinyint`
- `DataType::Int16` -> `LogicalTypeId::Smallint`
- `DataType::UInt8` -> `LogicalTypeId::Utinyint`
- `DataType::UInt16` -> `LogicalTypeId::Usmallint`
- `DataType::UInt32` -> `LogicalTypeId::Uinteger`
- `DataType::UInt64` -> `LogicalTypeId::Ubigint`

### 2.2 Buffer Enum Expansion
We will add seven new variants to `ChunkBuffer`:
```rust
pub enum ChunkBuffer {
    // ... existing ...
    Bool(Vec<bool>),
    Int8(Vec<i8>),
    Int16(Vec<i16>),
    UInt8(Vec<u8>),
    UInt16(Vec<u16>),
    UInt32(Vec<u32>),
    UInt64(Vec<u64>),
}
```

### 2.3 Fill Value and Dispatch
- **FillValueCmp**: We will add `impl_fill_value_cmp!` declarations for all the new integer types (`i8`, `i16`, `u8`, `u16`, `u32`, `u64`). We will write a custom `FillValueCmp` for `bool` that converts `true`/`false` to `1u8`/`0u8` arrays before comparing against the Zarr fill bytes.
- **Dispatch**: We will add seven new `match` arms inside `ReadZarrVTab::func` to match the data type and invoke `dispatch_yield_loop!` with the appropriate primitive and `ChunkBuffer` variant.

## 3. Spec Self-Review
1. **Placeholder scan:** No TBDs.
2. **Internal consistency:** The dispatch macro `dispatch_yield_loop!` uses `to_ne_bytes()` which is available on all standard rust primitives, so it will work seamlessly with the new integer types. `bool` will need the custom implementation as mentioned.
3. **Scope check:** Bounded exclusively to expanding the data type switch cases.
4. **Ambiguity check:** The mapping between Rust types, Zarr types, and DuckDB Logical Types is explicitly defined.
