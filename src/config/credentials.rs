use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::config::secure_store;

pub const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8974/callback";

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
    pub fn load_optional() -> Result<Option<Self>> {
        let Some(credentials) = secure_store::load_secret::<Self>(
            secure_store::CREDENTIALS_ENTRY,
            "SoundCloud app credentials",
        )?
        else {
            return Ok(None);
        };

        credentials.validate()?;

        Ok(Some(credentials))
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        secure_store::save_secret(
            secure_store::CREDENTIALS_ENTRY,
            "SoundCloud app credentials",
            self,
        )
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
