use anyhow::{Context, Result, anyhow};
use log::{debug, info};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};
use tokio::sync::mpsc;

use crate::app::{AppEvent, AppState, PlaybackIntent};

use super::projection::{MediaControlsState, MediaPlaybackState};

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
            self.controls
                .set_metadata(metadata_for_state(&state))
                .context("could not update native media metadata")?;
        }

        if self
            .last_state
            .as_ref()
            .map(|previous| !previous.playback_matches(&state))
            .unwrap_or(true)
        {
            self.controls
                .set_playback(playback_for_state(&state))
                .context("could not update native playback state")?;
        }

        self.last_state = Some(state);
        Ok(())
    }

    pub fn pump_main_thread(&mut self) -> Result<()> {
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
        cover_url: track.artwork_url.as_deref(),
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
    hwnd: *mut std::ffi::c_void,
    thread_id: u32,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "windows")]
impl HiddenWindow {
    fn new() -> Result<Self> {
        use std::{sync::mpsc, thread};

        use windows::{
            Win32::{
                Foundation::HWND,
                System::Threading::GetCurrentThreadId,
                UI::WindowsAndMessaging::{
                    CreateWindowExW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
                    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
                },
            },
            core::w,
        };

        let (sender, receiver) = mpsc::sync_channel(1);
        let join_handle = thread::spawn(move || unsafe {
            let thread_id = GetCurrentThreadId();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("soundcloud-tui-media-controls"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                None,
                None,
            );

            if hwnd.0 == 0 {
                let _ = sender.send(Err("could not create hidden Windows media controls window"));
                return;
            }

            let _ = sender.send(Ok((hwnd.0 as *mut std::ffi::c_void, thread_id)));

            let mut message = MSG::default();
            while GetMessageW(&mut message, HWND::default(), 0, 0).into() {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        });

        let (hwnd, thread_id) = receiver
            .recv()
            .context("could not receive hidden Windows media controls window handle")?
            .map_err(|error| anyhow!(error))?;

        Ok(Self {
            hwnd,
            thread_id,
            join_handle: Some(join_handle),
        })
    }

    fn hwnd(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

#[cfg(target_os = "windows")]
impl Drop for HiddenWindow {
    fn drop(&mut self) {
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT},
        };

        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }

        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}
