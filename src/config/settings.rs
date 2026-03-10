use std::fs;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::paths::AppPaths;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub show_help_on_startup: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            show_help_on_startup: true,
        }
    }
}

impl Settings {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.settings_file.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&paths.settings_file)?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let contents = toml::to_string_pretty(self)?;
        fs::write(&paths.settings_file, contents)?;
        Ok(())
    }
}

pub fn ensure_default_file(paths: &AppPaths) -> Result<()> {
    if paths.settings_file.exists() {
        return Ok(());
    }

    Settings::default().save(paths)?;

    Ok(())
}
