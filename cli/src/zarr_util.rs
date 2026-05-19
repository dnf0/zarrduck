use color_eyre::eyre::Result;
use opendal::{
    services::{Fs, Http},
    Operator,
};

pub async fn list_arrays(uri: &str) -> Result<Vec<String>> {
    let operator = if uri.starts_with("http") {
        Operator::new(Http::default().endpoint(uri))?.finish()
    } else {
        Operator::new(Fs::default().root(uri))?.finish()
    };

    let is_group = operator.is_exist(".zgroup").await.unwrap_or(false);
    let mut arrays = Vec::new();

    if is_group {
        let entries = operator.list("/").await?;
        for entry in entries {
            if entry.metadata().is_dir() {
                let path = entry.path();
                if operator
                    .is_exist(&format!("{}.zarray", path))
                    .await
                    .unwrap_or(false)
                {
                    arrays.push(path.trim_end_matches('/').to_string());
                }
            }
        }
    } else if operator.is_exist(".zarray").await.unwrap_or(false) {
        arrays.push("".to_string());
    }

    Ok(arrays)
}

pub async fn resolve_zarr_uri(uri: &str, is_json: bool) -> Result<String> {
    let arrays = list_arrays(uri).await?;

    if arrays.is_empty() {
        // Assume it's a direct array URI or unreadable, just pass it through
        return Ok(uri.to_string());
    }

    if arrays.len() == 1 && arrays[0].is_empty() {
        // It's exactly an array
        return Ok(uri.to_string());
    }

    // It is a group containing arrays
    if is_json {
        return Err(color_eyre::eyre::eyre!(
            "Provided URI '{}' is a Zarr Group containing multiple datasets ({:?}). Please provide the exact path to a dataset.",
            uri, arrays
        ));
    }

    let selection = inquire::Select::new(
        "The specified Zarr URI is a Group. Select a dataset to use:",
        arrays.clone(),
    )
    .with_page_size(10)
    .prompt()?;

    // Build the resolved URI
    let resolved = if uri.ends_with('/') {
        format!("{}{}", uri, selection)
    } else {
        format!("{}/{}", uri, selection)
    };

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_arrays() {
        let arrays = list_arrays("../climate_data.zarr").await.unwrap();
        println!("Found arrays: {:?}", arrays);
        // assert_eq!(arrays.len(), 4);
    }
}
