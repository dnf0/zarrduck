use crate::{config::EiderConfig, stac, ui, OutputFormat};
use color_eyre::eyre::{eyre, Result as EyreResult, WrapErr};

fn build_stac_query(
    collection: &str,
    bbox: Option<&String>,
    datetime: Option<&String>,
) -> EyreResult<serde_json::Value> {
    let mut payload = serde_json::json!({
        "collections": [collection],
        "limit": 10
    });

    if let Some(b) = bbox {
        let bbox_arr: Vec<f64> = b
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .collect::<Result<Vec<_>, _>>()
            .wrap_err("Failed to parse bbox coordinates as floats")?;
        if bbox_arr.len() == 4 {
            payload
                .as_object_mut()
                .unwrap()
                .insert("bbox".to_string(), serde_json::json!(bbox_arr));
        } else {
            return Err(eyre!(
                "bbox must be 4 comma-separated numbers (min_lon, min_lat, max_lon, max_lat)"
            ));
        }
    }

    if let Some(dt) = datetime {
        payload
            .as_object_mut()
            .unwrap()
            .insert("datetime".to_string(), serde_json::json!(dt));
    }
    Ok(payload)
}

fn is_supported_asset(asset: &serde_json::Value) -> bool {
    let t = asset.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let href = asset.get("href").and_then(|h| h.as_str()).unwrap_or("");

    let is_zarr = t.contains("zarr") || href.ends_with(".zarr") || href.contains(".zarr/");
    let is_cog = t.contains("tiff")
        || t.contains("cog")
        || href.ends_with(".tif")
        || href.ends_with(".tiff");

    is_zarr || is_cog
}

fn extract_assets(
    assets: &serde_json::Map<String, serde_json::Value>,
    found_uris: &mut Vec<String>,
    found_options: &mut Vec<String>,
) {
    for (_, asset) in assets {
        if is_supported_asset(asset) {
            if let Some(href) = asset.get("href").and_then(|h| h.as_str()) {
                let title = asset.get("title").and_then(|t| t.as_str()).unwrap_or(href);
                let mut desc = asset
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .replace('\n', " ");
                if desc.len() > 80 {
                    desc.truncate(77);
                    desc.push_str("...");
                }

                if desc.is_empty() {
                    found_options.push(format!("{} - {}", href, title));
                } else {
                    found_options.push(format!("{} - {} ({})", href, title, desc));
                }
                found_uris.push(href.to_string());
            }
        }
    }
}

fn parse_search_results(stac_response: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let mut found_uris = Vec::new();
    let mut found_options = Vec::new();

    // Check features (items)
    if let Some(features) = stac_response.get("features").and_then(|f| f.as_array()) {
        for feature in features {
            if let Some(assets) = feature.get("assets").and_then(|a| a.as_object()) {
                extract_assets(assets, &mut found_uris, &mut found_options);
            }
        }
    }

    // Check if the response itself is a collection with assets
    if let Some(assets) = stac_response.get("assets").and_then(|a| a.as_object()) {
        extract_assets(assets, &mut found_uris, &mut found_options);
    }

    (found_uris, found_options)
}

fn output_json_results(uris: &[String]) {
    let json_out = serde_json::json!({
        "status": "success",
        "uris": uris
    });
    println!("{}", json_out);
}

fn get_selected_api(
    api: Option<String>,
    resolved_output: &OutputFormat,
    config: &EiderConfig,
) -> EyreResult<String> {
    if let Some(a) = api {
        return Ok(a);
    }
    if resolved_output == &OutputFormat::Json {
        return Err(eyre!("--api is required when using --output=json"));
    }

    let providers = stac::get_stac_providers(config);
    let mut select = inquire::Select::new("Select a STAC Provider:", providers);
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

    Ok(selection.split(" - ").next().unwrap().to_string())
}

async fn get_selected_collection(
    client: &reqwest::Client,
    selected_api: &str,
    current_collection: Option<&String>,
    resolved_output: &OutputFormat,
) -> EyreResult<Option<String>> {
    if let Some(c) = current_collection {
        return Ok(Some(c.clone()));
    }

    let collections_url = if selected_api.ends_with("/collections") {
        selected_api.to_string()
    } else {
        format!("{}/collections", selected_api.trim_end_matches('/'))
    };

    let res = client
        .get(&collections_url)
        .send()
        .await
        .wrap_err("Failed to fetch collections from STAC API")?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(eyre!("STAC API returned {}: {}", status, text));
    }

    let collections_response: serde_json::Value = res
        .json()
        .await
        .wrap_err("Failed to parse collections response")?;

    let mut collection_options = Vec::new();
    let mut collection_ids = Vec::new();

    if let Some(collections) = collections_response
        .get("collections")
        .and_then(|c| c.as_array())
    {
        for col in collections {
            if let Some(id) = col.get("id").and_then(|id| id.as_str()) {
                let mut has_supported_data = false;
                if let Some(assets) = col.get("assets").and_then(|a| a.as_object()) {
                    has_supported_data = assets.values().any(is_supported_asset);
                }
                if !has_supported_data {
                    if let Some(item_assets) = col.get("item_assets").and_then(|a| a.as_object()) {
                        has_supported_data = item_assets.values().any(is_supported_asset);
                    }
                }
                if !has_supported_data {
                    continue; // Skip collections that don't declare Zarr or COG assets
                }

                let title = col.get("title").and_then(|t| t.as_str()).unwrap_or(id);
                let mut desc = col
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .replace('\n', " ");
                if desc.len() > 80 {
                    desc.truncate(77);
                    desc.push_str("...");
                }

                if desc.is_empty() {
                    collection_options.push(format!("{} - {}", id, title));
                } else {
                    collection_options.push(format!("{} - {} ({})", id, title, desc));
                }
                collection_ids.push(id.to_string());
            }
        }
    }

    if collection_ids.is_empty() {
        return Err(eyre!("No collections found at {}", collections_url));
    }
    if resolved_output == &OutputFormat::Json {
        let json_out = serde_json::json!({
            "status": "success",
            "collections": collection_ids
        });
        println!("{}", json_out);
        return Ok(None);
    }

    let mut select =
        inquire::Select::new("Select a STAC Collection to search:", collection_options)
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

    Ok(Some(selection.split(" - ").next().unwrap().to_string()))
}

pub async fn run_search(
    api: Option<String>,
    collection: Option<String>,
    bbox: Option<String>,
    datetime: Option<String>,
    resolved_output: &OutputFormat,
    config: &EiderConfig,
) -> EyreResult<()> {
    let client = reqwest::Client::new();
    let selected_api = get_selected_api(api, resolved_output, config)?;

    let mut current_collection = collection.clone();
    loop {
        let selected_collection = match get_selected_collection(
            &client,
            &selected_api,
            current_collection.as_ref(),
            resolved_output,
        )
        .await?
        {
            Some(c) => c,
            None => return Ok(()),
        };

        let payload = build_stac_query(&selected_collection, bbox.as_ref(), datetime.as_ref())?;

        let mut search_api = selected_api.clone();
        if !search_api.ends_with("/search") {
            search_api = format!("{}/search", search_api.trim_end_matches('/'));
        }

        if resolved_output != &OutputFormat::Json {
            println!("Querying STAC API: {}", search_api);
        }

        let res = client
            .post(&search_api)
            .json(&payload)
            .send()
            .await
            .wrap_err("Failed to send request to STAC API")?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(eyre!("STAC API returned {}: {}", status, text));
        }

        let mut stac_response: serde_json::Value = res
            .json()
            .await
            .wrap_err("Failed to parse STAC API response")?;

        // If the `/search` response returned no features (or it's a dataset where the zarr is attached to the collection),
        // let's fetch the collection itself to see if it has the assets.
        if let Some(features) = stac_response.get("features").and_then(|f| f.as_array()) {
            if features.is_empty() {
                // Fetch the collection specifically
                let collection_url = format!(
                    "{}/collections/{}",
                    selected_api.trim_end_matches('/'),
                    selected_collection
                );
                if let Ok(col_res) = client.get(&collection_url).send().await {
                    if let Ok(col_json) = col_res.json::<serde_json::Value>().await {
                        stac_response = col_json;
                    }
                }
            }
        }

        let (found_uris, found_options) = parse_search_results(&stac_response);

        if resolved_output == &OutputFormat::Json {
            output_json_results(&found_uris);
            break;
        } else if found_uris.is_empty() {
            println!(
                "No Zarr or COG URIs found in collection {}. Restarting selection loop...\n",
                selected_collection
            );
            current_collection = None;
            continue;
        } else {
            let selection = if found_options.len() == 1 {
                found_uris[0].clone()
            } else {
                let prompt_msg = format!(
                    "Found {} Data URIs. Select a dataset to use:",
                    found_options.len()
                );
                let mut select =
                    inquire::Select::new(&prompt_msg, found_options).with_page_size(10);
                select.scorer = &|input, _, string_value, _| {
                    let input = input.to_lowercase();
                    let val = string_value.to_lowercase();
                    if input.split_whitespace().all(|word| val.contains(word)) {
                        Some(1)
                    } else {
                        None
                    }
                };
                let chosen = select.prompt()?;
                chosen.split(" - ").next().unwrap().to_string()
            };

            // Resolve the specific channel/array from the Zarr group
            let resolved_uri = ui::prompt_zarr_uri(&selection, false).await?;

            println!("Selected Dataset: {}", resolved_uri);
            println!("You can now extract this data using:");
            println!("eider extract {} <your-vector-file.geojson>", resolved_uri);
            break;
        }
    }
    Ok(())
}
