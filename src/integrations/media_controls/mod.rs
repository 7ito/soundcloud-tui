use anyhow::Result;
use tokio::sync::mpsc;

use crate::app::{AppEvent, AppState};

pub(crate) mod projection;

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod native;

#[cfg(all(feature = "mpris", target_os = "linux"))]
pub struct MediaControlsIntegration {
    backend: super::mpris::MprisIntegration,
}

#[cfg(all(feature = "mpris", target_os = "linux"))]
impl MediaControlsIntegration {
    pub async fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Result<Option<Self>> {
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
