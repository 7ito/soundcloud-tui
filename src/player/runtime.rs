use std::{
    env,
    sync::OnceLock,
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use anyhow::Result;
use log::info;
use tokio::sync::mpsc as tokio_mpsc;

use crate::{
    app::AppEvent,
    config::paths::AppPaths,
    player::{
        backend::PlayerBackend, command::PlayerCommand, event::PlayerEvent, mpv::MpvPlayerBackend,
    },
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const LIMIT_PLAYER_EVENTS_PER_TICK_ENV_VAR: &str = "SOUNDCLOUD_TUI_LIMIT_PLAYER_EVENTS_PER_TICK";

#[derive(Clone)]
pub struct PlayerHandle {
    command_tx: mpsc::Sender<PlayerCommand>,
}

impl PlayerHandle {
    pub fn spawn(paths: AppPaths, app_events: tokio_mpsc::UnboundedSender<AppEvent>) -> Self {
        let (command_tx, command_rx) = mpsc::channel();

        thread::spawn(move || {
            let mut backend: Option<MpvPlayerBackend> = None;

            loop {
                match command_rx.recv_timeout(POLL_INTERVAL) {
                    Ok(command) => {
                        if matches!(command, PlayerCommand::Shutdown) {
                            break;
                        }

                        if let Err(error) = ensure_backend(&paths, &mut backend)
                            .and_then(|backend| backend.send(command))
                        {
                            backend = None;
                            let _ = app_events.send(AppEvent::Player(PlayerEvent::BackendError(
                                error.to_string(),
                            )));
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                let mut drop_backend = false;

                if let Some(player_backend) = backend.as_mut() {
                    loop {
                        match player_backend.poll_event() {
                            Ok(Some(event)) => {
                                let backend_failed = matches!(event, PlayerEvent::BackendError(_));
                                let _ = app_events.send(AppEvent::Player(event));
                                if backend_failed {
                                    drop_backend = true;
                                    break;
                                }

                                if should_limit_player_events_per_tick() {
                                    log_player_event_limit_once();
                                    break;
                                }
                            }
                            Ok(None) => break,
                            Err(error) => {
                                drop_backend = true;
                                let _ = app_events.send(AppEvent::Player(
                                    PlayerEvent::BackendError(error.to_string()),
                                ));
                                break;
                            }
                        }
                    }
                }

                if drop_backend {
                    backend = None;
                }
            }
        });

        Self { command_tx }
    }

    pub fn send(&self, command: PlayerCommand) -> Result<()> {
        self.command_tx.send(command)?;
        Ok(())
    }
}

fn ensure_backend<'a>(
    paths: &AppPaths,
    backend: &'a mut Option<MpvPlayerBackend>,
) -> Result<&'a mut MpvPlayerBackend> {
    if backend.is_none() {
        *backend = Some(MpvPlayerBackend::spawn(paths)?);
    }

    Ok(backend.as_mut().expect("backend initialized"))
}

fn should_limit_player_events_per_tick() -> bool {
    static LIMIT: OnceLock<bool> = OnceLock::new();

    *LIMIT.get_or_init(|| {
        env::var(LIMIT_PLAYER_EVENTS_PER_TICK_ENV_VAR)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}

fn log_player_event_limit_once() {
    static LOGGED: OnceLock<()> = OnceLock::new();

    let _ = LOGGED.get_or_init(|| {
        info!(
            "player event polling limited to one event per tick via {}",
            LIMIT_PLAYER_EVENTS_PER_TICK_ENV_VAR
        );
    });
}
