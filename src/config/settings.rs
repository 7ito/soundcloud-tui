use std::fs;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::paths::AppPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
        }
    }
}

pub fn ensure_default_file(paths: &AppPaths) -> Result<()> {
    if paths.settings_file.exists() {
        return Ok(());
    }

    let contents = toml::to_string_pretty(&Settings::default())?;
    fs::write(&paths.settings_file, contents)?;

    Ok(())
}
