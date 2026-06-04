use std::io::IsTerminal;
use owo_colors::OwoColorize;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Agent,
    AgentJson,
}

#[allow(dead_code)]
impl OutputMode {
    pub fn detect(json_requested: bool) -> Self {
        if json_requested {
            OutputMode::AgentJson
        } else if std::io::stdout().is_terminal() {
            OutputMode::Human
        } else {
            OutputMode::Agent
        }
    }

    pub fn is_human(&self) -> bool {
        *self == OutputMode::Human
    }
}

#[allow(dead_code)]
pub fn format_key(key: &str, mode: OutputMode) -> String {
    if mode.is_human() {
        key.cyan().to_string()
    } else {
        key.to_string()
    }
}

#[allow(dead_code)]
pub fn format_value(val: &str, mode: OutputMode) -> String {
    if mode.is_human() {
        val.magenta().to_string()
    } else {
        val.to_string()
    }
}

#[allow(dead_code)]
pub fn format_success(msg: &str, mode: OutputMode) -> String {
    if mode.is_human() {
        format!("{} {}", "✔".green(), msg)
    } else {
        format!("- SUCCESS: {}", msg)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use owo_colors::OwoColorize;

    #[test]
    fn test_detect_json() {
        assert_eq!(OutputMode::detect(true), OutputMode::AgentJson);
    }
    
    #[test]
    fn test_format_key() {
        assert_eq!(format_key("test", OutputMode::Agent), "test");
        assert_eq!(format_key("test", OutputMode::AgentJson), "test");
        assert_eq!(format_key("test", OutputMode::Human), "test".cyan().to_string());
    }

    #[test]
    fn test_format_value() {
        assert_eq!(format_value("val", OutputMode::Agent), "val");
        assert_eq!(format_value("val", OutputMode::AgentJson), "val");
        assert_eq!(format_value("val", OutputMode::Human), "val".magenta().to_string());
    }

    #[test]
    fn test_format_success() {
        assert_eq!(format_success("done", OutputMode::Agent), "- SUCCESS: done");
        assert_eq!(format_success("done", OutputMode::AgentJson), "- SUCCESS: done");
        assert_eq!(format_success("done", OutputMode::Human), format!("{} done", "✔".green()));
    }
}
