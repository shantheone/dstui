use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub accept_invalid_certs: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SortConfig {
    #[serde(default)]
    pub column: String, // "name", "size", "progress", etc.
    #[serde(default)]
    pub order: String, // "ascending" or "descending"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub connection: ConnectionConfig,
    pub downloads: DownloadConfig,
    #[serde(default)]
    pub sorting: SortConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub destination: String,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: Option<u64>, // in seconds, None = disabled
}

fn default_refresh_interval() -> Option<u64> {
    Some(30)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            connection: ConnectionConfig {
                url: String::from("http://your-diskstation:5000"),
                username: String::from("admin"),
                password: String::new(),
                accept_invalid_certs: false,
            },
            downloads: DownloadConfig {
                destination: String::from("downloads"),
                refresh_interval: Some(30),
            },
            sorting: SortConfig {
                column: String::from("name"),
                order: String::from("ascending"),
            },
        }
    }
}

pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("dstui");
    Ok(config_dir.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;

    if !path.exists() {
        anyhow::bail!("no_config"); // sentinel value
    }

    let contents = std::fs::read_to_string(&path)
        .context(format!("Failed to read config file at {}", path.display()))?;
    let config: Config = toml::from_str(&contents)
        .context("Failed to parse config file — check your TOML syntax")?;

    Ok(config)
}
