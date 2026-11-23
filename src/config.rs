use crate::constants::APP_NAME;
use crate::error::AppError;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub default_download_path: PathBuf,
    pub show_hidden_files: bool,
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_download_path: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            show_hidden_files: false,
            theme: String::from("dark"),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)?;
        toml::from_str(&raw)
            .map_err(|error| AppError::Config(format!("Could not read config: {error}")))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let raw = toml::to_string_pretty(self)
            .map_err(|error| AppError::Config(format!("Could not write config: {error}")))?;
        fs::write(path, raw)?;
        Ok(())
    }
}

fn config_path() -> PathBuf {
    let dir_name = APP_NAME.to_ascii_lowercase();
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(dir_name)
        .join("config.toml")
}
