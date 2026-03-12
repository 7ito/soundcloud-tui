use std::{fs, path::PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub settings_file: PathBuf,
    pub history_file: PathBuf,
    pub log_file: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let app_name = "soundcloud-tui";

        let config_dir = dirs::config_dir()
            .context("could not determine config directory")?
            .join(app_name);
        let state_dir =
            resolve_state_base_dir(dirs::state_dir(), dirs::data_local_dir())?.join(app_name);
        let cache_dir = dirs::cache_dir()
            .context("could not determine cache directory")?
            .join(app_name);

        Ok(Self {
            settings_file: config_dir.join("settings.toml"),
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

fn resolve_state_base_dir(
    state_dir: Option<PathBuf>,
    local_data_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    state_dir
        .or(local_data_dir)
        .context("could not determine state directory or local data directory")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::resolve_state_base_dir;

    #[test]
    fn state_resolution_prefers_state_directory_when_available() {
        let state_dir = PathBuf::from("/tmp/state");
        let local_data_dir = PathBuf::from("/tmp/local-data");

        let resolved = resolve_state_base_dir(Some(state_dir.clone()), Some(local_data_dir))
            .expect("state directory should resolve");

        assert_eq!(resolved, state_dir);
    }

    #[test]
    fn state_resolution_falls_back_to_local_data_directory() {
        let local_data_dir = PathBuf::from("/tmp/local-data");

        let resolved = resolve_state_base_dir(None, Some(local_data_dir.clone()))
            .expect("local data fallback should resolve");

        assert_eq!(resolved, local_data_dir);
    }

    #[test]
    fn state_resolution_errors_when_no_base_directory_exists() {
        let error = resolve_state_base_dir(None, None).expect_err("missing dirs should error");

        assert!(
            error
                .to_string()
                .contains("state directory or local data directory")
        );
    }
}
