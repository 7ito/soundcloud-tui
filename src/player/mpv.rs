use std::{
    fs,
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
        if socket_path.exists() {
            let _ = fs::remove_file(&socket_path);
        }

        let child = Command::new("mpv")
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
            .context("could not spawn mpv - make sure it is installed and on PATH")?;

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
                        "mpv IPC socket closed unexpectedly".to_string(),
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
        let _ = fs::remove_file(&self.socket_path);
    }
}

fn socket_path(paths: &AppPaths) -> PathBuf {
    paths
        .cache_dir
        .join(format!("mpv-{}.sock", std::process::id()))
}

fn connect_ipc(socket_path: &Path) -> Result<IpcClient> {
    let started = Instant::now();

    loop {
        match IpcClient::connect(socket_path) {
            Ok(ipc) => return Ok(ipc),
            Err(error) if started.elapsed() < SOCKET_CONNECT_TIMEOUT => {
                thread::sleep(SOCKET_CONNECT_RETRY);
                if !socket_path.exists() {
                    continue;
                }
                let _ = error;
            }
            Err(error) => {
                return Err(anyhow!(error))
                    .context("timed out waiting for mpv IPC socket to become available");
            }
        }
    }
}
