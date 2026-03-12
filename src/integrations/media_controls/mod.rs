use std::env;

use anyhow::Result;
use log::info;
use tokio::sync::mpsc;

use crate::app::{AppEvent, AppState};

pub(crate) mod projection;

pub(crate) const DISABLE_MEDIA_CONTROLS_ENV_VAR: &str = "SOUNDCLOUD_TUI_DISABLE_MEDIA_CONTROLS";
pub(crate) const DISABLE_MEDIA_ARTWORK_ENV_VAR: &str = "SOUNDCLOUD_TUI_DISABLE_MEDIA_ARTWORK";

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native;

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub struct MediaControlsIntegration {
    backend: super::mpris::MprisIntegration,
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
impl MediaControlsIntegration {
    pub async fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Result<Option<Self>> {
        if media_controls_disabled() {
            info!(
                "media controls integration disabled via {}",
                DISABLE_MEDIA_CONTROLS_ENV_VAR
            );
            return Ok(None);
        }

        Ok(Some(Self {
            backend: super::mpris::MprisIntegration::new(sender).await?,
        }))
    }

    pub async fn sync_from_app(&mut self, app: &AppState) -> Result<()> {
        self.backend.sync_from_app(app).await
    }

    pub fn pump_main_thread(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub struct MediaControlsIntegration {
    backend: native::NativeMediaControls,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl MediaControlsIntegration {
    pub async fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Result<Option<Self>> {
        if media_controls_disabled() {
            info!(
                "media controls integration disabled via {}",
                DISABLE_MEDIA_CONTROLS_ENV_VAR
            );
            return Ok(None);
        }

        Ok(Some(Self {
            backend: native::NativeMediaControls::new(sender)?,
        }))
    }

    pub async fn sync_from_app(&mut self, app: &AppState) -> Result<()> {
        self.backend.sync_from_app(app)
    }

    pub fn pump_main_thread(&mut self) -> Result<()> {
        self.backend.pump_main_thread()
    }
}

#[cfg(not(any(
    all(feature = "mpris", target_os = "linux"),
    target_os = "windows",
    target_os = "macos"
)))]
pub struct MediaControlsIntegration;

#[cfg(not(any(
    all(feature = "mpris", target_os = "linux"),
    target_os = "windows",
    target_os = "macos"
)))]
impl MediaControlsIntegration {
    pub async fn new(_sender: mpsc::UnboundedSender<AppEvent>) -> Result<Option<Self>> {
        Ok(None)
    }

    pub async fn sync_from_app(&mut self, _app: &AppState) -> Result<()> {
        Ok(())
    }

    pub fn pump_main_thread(&mut self) -> Result<()> {
        Ok(())
    }
}

pub(crate) fn media_artwork_disabled() -> bool {
    env_var_enabled(DISABLE_MEDIA_ARTWORK_ENV_VAR)
}

fn media_controls_disabled() -> bool {
    env_var_enabled(DISABLE_MEDIA_CONTROLS_ENV_VAR)
}

fn env_var_enabled(name: &str) -> bool {
    env::var(name)
        .map(|value| env_var_truthy(&value))
        .unwrap_or(false)
}

fn env_var_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::env_var_truthy;

    #[test]
    fn truthy_env_values_are_case_insensitive() {
        assert!(env_var_truthy("1"));
        assert!(env_var_truthy("true"));
        assert!(env_var_truthy(" YES "));
        assert!(env_var_truthy("On"));
    }

    #[test]
    fn falsy_env_values_are_rejected() {
        assert!(!env_var_truthy("0"));
        assert!(!env_var_truthy("false"));
        assert!(!env_var_truthy("disabled"));
        assert!(!env_var_truthy(""));
    }
}
