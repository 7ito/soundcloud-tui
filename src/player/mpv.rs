use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    config::paths::AppPaths,
    player::{
        backend::PlayerBackend,
        command::PlayerCommand,
        event::PlayerEvent,
        ipc::{IpcClient, IpcMessage},
        mpv_locator,
    },
};

const OBSERVE_PAUSE: u64 = 1;
const OBSERVE_PLAYBACK_TIME: u64 = 2;
const OBSERVE_DURATION: u64 = 3;
const OBSERVE_VOLUME: u64 = 4;
const SOCKET_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const SOCKET_CONNECT_RETRY: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub struct MpvPlayerBackend {
    child: Child,
    ipc: IpcClient,
    socket_path: PathBuf,
    exit_reported: bool,
}

impl MpvPlayerBackend {
    pub fn spawn(paths: &AppPaths) -> Result<Self> {
        let socket_path = socket_path(paths);
        cleanup_socket_path(&socket_path);
        let mpv_path = mpv_locator::discover()?;

        let child = Command::new(&mpv_path)
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--audio-display=no")
            .arg("--force-window=no")
            .arg("--really-quiet")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| mpv_locator::launch_failed(mpv_path.clone(), error))?;

        let mut ipc = connect_ipc(&socket_path)?;
        ipc.observe_property(OBSERVE_PAUSE, "pause")?;
        ipc.observe_property(OBSERVE_PLAYBACK_TIME, "playback-time")?;
        ipc.observe_property(OBSERVE_DURATION, "duration")?;
        ipc.observe_property(OBSERVE_VOLUME, "volume")?;

        Ok(Self {
            child,
            ipc,
            socket_path,
            exit_reported: false,
        })
    }

    fn child_exit_event(&mut self) -> Result<Option<PlayerEvent>> {
        if let Some(status) = self.child.try_wait()? {
            if self.exit_reported {
                return Ok(None);
            }

            self.exit_reported = true;
            return Ok(Some(PlayerEvent::BackendError(format!(
                "mpv exited unexpectedly with status {status}"
            ))));
        }

        Ok(None)
    }
}

impl PlayerBackend for MpvPlayerBackend {
    fn send(&mut self, command: PlayerCommand) -> Result<()> {
        if let Some(event) = self.child_exit_event()? {
            bail!(match event {
                PlayerEvent::BackendError(message) => message,
                _ => "mpv backend unavailable".to_string(),
            });
        }

        self.ipc.send_command(command)
    }

    fn poll_event(&mut self) -> Result<Option<PlayerEvent>> {
        if let Some(event) = self.child_exit_event()? {
            return Ok(Some(event));
        }

        loop {
            match self.ipc.poll_message()? {
                Some(IpcMessage::Event(event)) => {
                    if let Some(player_event) = event.into_player_event() {
                        return Ok(Some(player_event));
                    }
                }
                Some(IpcMessage::Response(response)) => {
                    if let Some(error) = response.error.filter(|error| error != "success") {
                        return Ok(Some(PlayerEvent::BackendError(format!(
                            "mpv command failed: {error}"
                        ))));
                    }
                }
                Some(IpcMessage::Closed) => {
                    return Ok(Some(PlayerEvent::BackendError(
                        "mpv IPC connection closed unexpectedly".to_string(),
                    )));
                }
                None => return Ok(None),
            }
        }
    }
}

impl Drop for MpvPlayerBackend {
    fn drop(&mut self) {
        let _ = self.ipc.send_command(PlayerCommand::Shutdown);
        let _ = self.child.kill();
        let _ = self.child.wait();
        cleanup_socket_path(&self.socket_path);
    }
}

fn socket_path(paths: &AppPaths) -> PathBuf {
    #[cfg(windows)]
    {
        return PathBuf::from(format!(
            r"\\.\pipe\soundcloud-tui-mpv-{}",
            std::process::id()
        ));
    }

    #[cfg(not(windows))]
    paths
        .cache_dir
        .join(format!("mpv-{}.sock", std::process::id()))
}

fn cleanup_socket_path(socket_path: &Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(socket_path);
    }

    #[cfg(not(unix))]
    {
        let _ = socket_path;
    }
}

fn connect_ipc(socket_path: &Path) -> Result<IpcClient> {
    let started = Instant::now();

    loop {
        match IpcClient::connect(socket_path) {
            Ok(ipc) => return Ok(ipc),
            Err(error) if started.elapsed() < SOCKET_CONNECT_TIMEOUT => {
                let _ = error;
                thread::sleep(SOCKET_CONNECT_RETRY);
            }
            Err(error) => {
                return Err(anyhow!(error))
                    .context("timed out waiting for mpv IPC endpoint to become available");
            }
        }
    }
}
