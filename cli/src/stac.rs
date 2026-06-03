use crate::config::EiderConfig;

pub fn get_stac_providers(config: &EiderConfig) -> Vec<String> {
    let mut providers = vec![
        "https://planetarycomputer.microsoft.com/api/stac/v1 - Microsoft Planetary Computer"
            .to_string(),
        "https://earth-search.aws.element84.com/v1 - Earth Search (Element84/AWS)".to_string(),
        "https://api.pangeo-forge.org/stac/ - Pangeo Forge".to_string(),
    ];
    if let Some(local_stac) = &config.local_stac {
        providers.push(format!("{} - Local STAC", local_stac));
    }
    providers
}
