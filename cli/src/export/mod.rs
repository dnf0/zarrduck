pub mod metadata;
pub mod schema;
pub mod stream;

use color_eyre::eyre::Result as EyreResult;
use duckdb::Connection;
pub use metadata::MetadataBuilder;
pub use schema::SchemaInferencer;
pub use stream::StreamWriter;

pub async fn run_export(
    conn: &Connection,
    query: &str,
    output: &str,
    value_column: &str,
    chunks: Option<String>,
    is_json: bool,
) -> EyreResult<()> {
    if !is_json {
        println!("Exporting to Zarr...");
        println!("Query: {}", query);
        println!("Output: {}", output);
        println!("Value Column: {}", value_column);
        if let Some(c) = &chunks {
            println!("Chunks: {}", c);
        }
    }

    let inferencer = SchemaInferencer {
        conn,
        query,
        value_column,
    };

    let (_all_columns, coord_columns) = inferencer.get_columns()?;

    if !is_json {
        println!("Pass 1: Inferring shape...");
    }
    let shape = inferencer.infer_shape(&coord_columns)?;
    if !is_json {
        println!("Inferred Shape: {:?}", shape);
    }

    let data_type = inferencer.infer_type()?;

    let builder = MetadataBuilder {
        output,
        shape: shape.clone(),
        data_type: data_type.clone(),
        coord_columns: coord_columns.clone(),
        chunks,
    };

    let (array, chunk_shape) = builder.build_array().await?;

    if !is_json {
        println!("Initialized Zarr Array.");
        println!("Pass 2: Streaming data...");
    }

    let writer = StreamWriter {
        conn,
        query,
        value_column,
        coord_columns,
        shape,
        chunk_shape,
        data_type,
        array,
        is_json,
    };

    writer.stream_data().await?;

    if !is_json {
        println!("Export successful!");
    }

    Ok(())
}
