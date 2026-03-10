use std::fs;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::paths::AppPaths;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at_epoch: i64,
}

impl TokenStore {
    pub fn load(paths: &AppPaths) -> Result<Option<Self>> {
        if !paths.tokens_file.exists() {
            return Ok(None);
        }

        let raw = fs::read_to_string(&paths.tokens_file)?;
        let tokens = serde_json::from_str(&raw)?;
        Ok(Some(tokens))
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&paths.tokens_file, raw)?;
        Ok(())
    }

    pub fn clear(paths: &AppPaths) -> Result<()> {
        if paths.tokens_file.exists() {
            fs::remove_file(&paths.tokens_file)?;
        }
        Ok(())
    }

    pub fn expires_soon(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at_epoch <= now + 60
    }

    pub fn has_refresh_token(&self) -> bool {
        !self.refresh_token.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::TokenStore;

    #[test]
    fn expires_soon_recognizes_future_tokens() {
        let tokens = TokenStore {
            expires_at_epoch: chrono::Utc::now().timestamp() + 3600,
            ..TokenStore::default()
        };

        assert!(!tokens.expires_soon());
    }
}
