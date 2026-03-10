use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub settings_file: PathBuf,
    pub credentials_file: PathBuf,
    pub tokens_file: PathBuf,
    pub history_file: PathBuf,
    pub log_file: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let app_name = "soundcloud-tui";

        let config_dir = dirs::config_dir()
            .context("could not determine config directory")?
            .join(app_name);
        let state_dir = dirs::state_dir()
            .context("could not determine state directory")?
            .join(app_name);
        let cache_dir = dirs::cache_dir()
            .context("could not determine cache directory")?
            .join(app_name);

        Ok(Self {
            settings_file: config_dir.join("settings.toml"),
            credentials_file: config_dir.join("credentials.toml"),
            tokens_file: config_dir.join("tokens.json"),
            history_file: state_dir.join("history.json"),
            log_file: state_dir.join("soundcloud-tui.log"),
            config_dir,
            state_dir,
            cache_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.state_dir)?;
        fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }
}
