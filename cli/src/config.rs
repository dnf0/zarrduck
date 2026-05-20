use serde::Deserialize;
use figment::{Figment, providers::{Format, Toml, Env}};
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Default)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ZarrduckConfig {
    pub output_format: Option<String>,
    pub default_out: Option<String>,
    pub local_stac: Option<String>,
    pub s3: Option<S3Config>,
}

impl ZarrduckConfig {
    pub fn load() -> color_eyre::eyre::Result<Self> {
        let mut figment = Figment::new();

        // Global config
        if let Some(proj_dirs) = ProjectDirs::from("", "", "zarrduck") {
            let global_config = proj_dirs.config_dir().join("config.toml");
            if global_config.exists() {
                figment = figment.merge(Toml::file(global_config));
            }
        }

        // Local config
        let local_config = std::env::current_dir().unwrap_or_default().join(".zarrduck.toml");
        if local_config.exists() {
            figment = figment.merge(Toml::file(local_config));
        }

        figment = figment.merge(Env::prefixed("ZARRDUCK_"));

        let config: ZarrduckConfig = figment.extract()?;

        Ok(config)
    }
}
