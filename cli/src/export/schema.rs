use color_eyre::eyre::{eyre, Result as EyreResult};
use duckdb::Connection;

pub struct SchemaInferencer<'a> {
    pub conn: &'a Connection,
    pub query: &'a str,
    pub value_column: &'a str,
}

impl<'a> SchemaInferencer<'a> {
    pub fn get_columns(&self) -> EyreResult<(Vec<String>, Vec<String>)> {
        let query_info = format!("DESCRIBE {}", self.query);
        let mut info_stmt = self.conn.prepare(&query_info)?;
        let mut rows = info_stmt.query([])?;

        let mut all_columns = Vec::new();
        let mut coord_columns = Vec::new();

        while let Some(row) = rows.next()? {
            let col_name: String = row.get(0)?;
            all_columns.push(col_name.clone());
            if col_name != self.value_column {
                coord_columns.push(col_name);
            }
        }

        if !all_columns.contains(&self.value_column.to_string()) {
            return Err(eyre!(
                "Value column '{}' not found in query results",
                self.value_column
            ));
        }

        Ok((all_columns, coord_columns))
    }

    pub fn infer_shape(&self, coord_columns: &[String]) -> EyreResult<Vec<u64>> {
        let mut shape = Vec::new();

        if !coord_columns.is_empty() {
            let mut agg_selects = Vec::new();
            for coord in coord_columns {
                agg_selects.push(format!(
                    "COUNT(DISTINCT \"{}\")",
                    coord.replace("\"", "\"\"")
                ));
            }

            let inference_query = format!(
                "SELECT {} FROM ({}) AS _geozarr_subq",
                agg_selects.join(", "),
                self.query
            );
            let mut inf_stmt = self.conn.prepare(&inference_query)?;

            inf_stmt.query_row([], |row| {
                for i in 0..coord_columns.len() {
                    let count: u64 = row.get(i)?;
                    shape.push(count);
                }
                Ok(())
            })?;
        }

        Ok(shape)
    }

    pub fn infer_type(&self) -> EyreResult<zarrs::array::DataType> {
        let query_info = format!("DESCRIBE {}", self.query);
        let mut type_stmt = self.conn.prepare(&query_info)?;
        let mut t_rows = type_stmt.query([])?;
        let mut value_type_str = "FLOAT".to_string();

        while let Some(row) = t_rows.next()? {
            let col_name: String = row.get(0)?;
            if col_name == self.value_column {
                value_type_str = row.get(1)?;
            }
        }

        let data_type = geozarr_core::types::string_to_zarr_type(&value_type_str)
            .map_err(|e| eyre!("{}", e))?;

        Ok(data_type)
    }
}
