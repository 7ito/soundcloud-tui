use std::ffi::c_void;

use anyhow::{Context, Result};
use log::{debug, info};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tokio::sync::mpsc;

use crate::app::{AppEvent, AppState, PlaybackIntent};

use super::{
    DISABLE_MEDIA_ARTWORK_ENV_VAR, media_artwork_disabled,
    projection::{MediaControlsState, MediaPlaybackState},
};

const DISPLAY_NAME: &str = "soundcloud-tui";
const DBUS_NAME: &str = "io.github.tito.soundcloud_tui";

pub struct NativeMediaControls {
    controls: MediaControls,
    last_state: Option<MediaControlsState>,
    #[cfg(target_os = "windows")]
    window: HiddenWindow,
}

impl NativeMediaControls {
    pub fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Result<Self> {
        #[cfg(target_os = "macos")]
        bootstrap_macos_application()?;

        #[cfg(target_os = "windows")]
        let window = HiddenWindow::new()?;

        let mut controls = MediaControls::new(PlatformConfig {
            display_name: DISPLAY_NAME,
            dbus_name: DBUS_NAME,
            #[cfg(target_os = "windows")]
            hwnd: Some(window.hwnd()),
            #[cfg(not(target_os = "windows"))]
            hwnd: None,
        })
        .context("could not initialize native media controls")?;

        controls
            .attach(move |event| handle_native_event(&sender, event))
            .context("could not attach native media controls callbacks")?;

        #[cfg(target_os = "windows")]
        info!("registered soundcloud-tui with Windows media controls");

        if media_artwork_disabled() {
            info!(
                "native media artwork sync disabled via {}",
                DISABLE_MEDIA_ARTWORK_ENV_VAR
            );
        }

        #[cfg(target_os = "macos")]
        info!("registered soundcloud-tui with macOS media controls");

        Ok(Self {
            controls,
            last_state: None,
            #[cfg(target_os = "windows")]
            window,
        })
    }

    pub fn sync_from_app(&mut self, app: &AppState) -> Result<()> {
        let state = MediaControlsState::from_app(app);

        if self
            .last_state
            .as_ref()
            .map(|previous| !previous.metadata_matches(&state))
            .unwrap_or(true)
        {
            let metadata = metadata_for_state(&state);
            info!(
                "syncing native media metadata: track={:?}, artwork_url_present={}",
                state.track.as_ref().map(|track| track.title.as_str()),
                metadata.cover_url.is_some()
            );
            self.controls
                .set_metadata(metadata)
                .context("could not update native media metadata")?;
            info!("native media metadata synced successfully");
        }

        if self
            .last_state
            .as_ref()
            .map(|previous| !previous.playback_matches(&state))
            .unwrap_or(true)
        {
            let playback = playback_for_state(&state);
            let should_log = self
                .last_state
                .as_ref()
                .map(|previous| {
                    previous.playback != state.playback
                        || previous.position.is_none() != state.position.is_none()
                })
                .unwrap_or(true);
            if should_log {
                info!(
                    "syncing native playback state: playback={:?}, position={:?}",
                    state.playback, state.position
                );
            }
            self.controls
                .set_playback(playback)
                .context("could not update native playback state")?;
            if should_log {
                info!("native playback state synced successfully");
            }
        }

        self.last_state = Some(state);
        Ok(())
    }

    pub fn pump_main_thread(&mut self) -> Result<()> {
        #[cfg(target_os = "windows")]
        pump_windows_event_queue();

        Ok(())
    }
}

fn handle_native_event(sender: &mpsc::UnboundedSender<AppEvent>, event: MediaControlEvent) {
    let intent = match event {
        MediaControlEvent::Play => Some(PlaybackIntent::Play),
        MediaControlEvent::Pause => Some(PlaybackIntent::Pause),
        MediaControlEvent::Toggle => Some(PlaybackIntent::TogglePause),
        MediaControlEvent::Next => Some(PlaybackIntent::Next),
        MediaControlEvent::Previous => Some(PlaybackIntent::Previous),
        MediaControlEvent::Stop => Some(PlaybackIntent::Stop),
        other => {
            debug!("ignoring unsupported native media event: {:?}", other);
            None
        }
    };

    if let Some(intent) = intent {
        let _ = sender.send(AppEvent::PlaybackIntent(intent));
    }
}

fn metadata_for_state(state: &MediaControlsState) -> MediaMetadata<'_> {
    let Some(track) = state.track.as_ref() else {
        return MediaMetadata::default();
    };

    MediaMetadata {
        title: Some(track.title.as_str()),
        album: None,
        artist: Some(track.artist.as_str()),
        cover_url: (!media_artwork_disabled())
            .then_some(track.artwork_url.as_deref())
            .flatten(),
        duration: track.duration,
    }
}

fn playback_for_state(state: &MediaControlsState) -> MediaPlayback {
    let progress = state.position.map(MediaPosition);

    match state.playback {
        MediaPlaybackState::Stopped => MediaPlayback::Stopped,
        MediaPlaybackState::Paused => MediaPlayback::Paused { progress },
        MediaPlaybackState::Playing => MediaPlayback::Playing { progress },
    }
}

#[cfg(target_os = "macos")]
fn bootstrap_macos_application() -> Result<()> {
    use cocoa::{
        appkit::{NSApp, NSApplication, NSApplicationActivationPolicyProhibited},
        base::{id, nil},
    };

    unsafe {
        let app = NSApp();
        let app: id = if app == nil {
            NSApplication::sharedApplication(nil)
        } else {
            app
        };
        app.setActivationPolicy_(NSApplicationActivationPolicyProhibited);
    }

    Ok(())
}

#[cfg(target_os = "windows")]
struct HiddenWindow {
    hwnd: *mut c_void,
}

#[cfg(target_os = "windows")]
impl HiddenWindow {
    fn new() -> Result<Self> {
        use windows::{
            Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE,
            },
            core::w,
        };

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("soundcloud-tui-media-controls"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .context("could not create hidden Windows media controls window")?
        };

        Ok(Self {
            hwnd: hwnd.0 as *mut c_void,
        })
    }

    fn hwnd(&self) -> *mut c_void {
        self.hwnd
    }
}

#[cfg(target_os = "windows")]
impl Drop for HiddenWindow {
    fn drop(&mut self) {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::DestroyWindow};

        unsafe {
            let _ = DestroyWindow(HWND(self.hwnd as isize));
        }
    }
}

#[cfg(target_os = "windows")]
fn pump_windows_event_queue() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
    };

    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).into() {
            if message.message == WM_QUIT {
                break;
            }

            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
