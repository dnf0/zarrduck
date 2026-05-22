use color_eyre::eyre::{eyre, Result};

pub async fn prompt_zarr_uri(uri: &str, is_json: bool) -> Result<String> {
    let arrays = geozarr_core::store::list_arrays(uri)
        .await
        .map_err(|e| eyre!("{e}"))?;

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
        return Err(eyre!(
            "Provided URI '{}' is a Zarr Group containing multiple datasets ({:?}). Please provide the exact path to a dataset.",
            uri, arrays
        ));
    }

    let mut select = inquire::Select::new(
        "The specified Zarr URI is a Group. Select a dataset to use:",
        arrays.clone(),
    )
    .with_page_size(10);
    select.scorer = &|input, _, string_value, _| {
        let input = input.to_lowercase();
        let val = string_value.to_lowercase();
        if input.split_whitespace().all(|word| val.contains(word)) {
            Some(1)
        } else {
            None
        }
    };
    let selection = select.prompt()?;

    // Build the resolved URI
    let resolved = if uri.ends_with('/') {
        format!("{}{}", uri, selection)
    } else {
        format!("{}/{}", uri, selection)
    };

    Ok(resolved)
}
