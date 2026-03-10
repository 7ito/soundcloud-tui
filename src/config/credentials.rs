use std::fs;

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::config::paths::AppPaths;

pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8974/callback";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl Default for Credentials {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            redirect_uri: DEFAULT_REDIRECT_URI.to_string(),
        }
    }
}

impl Credentials {
    pub fn load_optional(paths: &AppPaths) -> Result<Option<Self>> {
        if !paths.credentials_file.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&paths.credentials_file)?;
        let credentials: Self = toml::from_str(&raw)
            .map_err(|error| anyhow!("invalid credentials file format: {error}"))?;

        credentials.validate()?;

        Ok(Some(credentials))
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        self.validate()?;
        let raw = toml::to_string_pretty(self)?;
        fs::write(&paths.credentials_file, raw)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.client_id.trim().is_empty() {
            bail!("Client ID is required.");
        }

        if self.client_secret.trim().is_empty() {
            bail!("Client secret is required.");
        }

        if self.redirect_uri.trim().is_empty() {
            bail!("Redirect URI is required.");
        }

        url::Url::parse(&self.redirect_uri)
            .map_err(|error| anyhow!("Redirect URI is invalid: {error}"))?;

        Ok(())
    }
}
