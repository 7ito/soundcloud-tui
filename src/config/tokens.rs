use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{paths::AppPaths, secure_store};

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at_epoch: i64,
}

impl TokenStore {
    pub fn load(paths: &AppPaths) -> Result<Option<Self>> {
        let Some(tokens) = secure_store::load_secret::<Self>(
            paths,
            secure_store::TOKENS_ENTRY,
            "SoundCloud session tokens",
        )?
        else {
            return Ok(None);
        };

        Ok(Some(tokens))
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        secure_store::save_secret(
            paths,
            secure_store::TOKENS_ENTRY,
            "SoundCloud session tokens",
            self,
        )
    }

    pub fn clear(paths: &AppPaths) -> Result<()> {
        secure_store::delete_secret(
            paths,
            secure_store::TOKENS_ENTRY,
            "SoundCloud session tokens",
        )
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
