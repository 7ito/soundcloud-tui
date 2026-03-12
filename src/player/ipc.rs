use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

#[cfg(windows)]
use log::info;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(windows)]
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

#[cfg(windows)]
use interprocess::os::windows::named_pipe::{
    DuplexPipeStream, RecvPipeStream, SendPipeStream, pipe_mode,
};

use crate::player::{command::PlayerCommand, event::PlayerEvent};

#[derive(Debug)]
pub struct IpcClient {
    connection: IpcConnection,
    read_buffer: Vec<u8>,
    request_id: u64,
}

#[derive(Debug)]
enum IpcConnection {
    #[cfg(unix)]
    Unix(UnixStream),
    #[cfg(windows)]
    Windows(WindowsPipeConnection),
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsPipeConnection {
    writer: SendPipeStream<pipe_mode::Bytes>,
    reader_rx: Receiver<std::result::Result<IpcMessage, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IpcMessage {
    Event(IpcEvent),
    Response(IpcResponse),
    Closed,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IpcEvent {
    PlaybackRestart,
    EndFile { reason: Option<String> },
    PropertyChange { name: String, data: Option<Value> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct IpcResponse {
    pub request_id: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawIpcMessage {
    event: Option<String>,
    name: Option<String>,
    data: Option<Value>,
    reason: Option<String>,
    request_id: Option<u64>,
    error: Option<String>,
}

impl IpcClient {
    pub fn connect(socket_path: &Path) -> Result<Self> {
        let connection = IpcConnection::connect(socket_path)?;

        Ok(Self {
            connection,
            read_buffer: Vec::new(),
            request_id: 1,
        })
    }

    pub fn observe_property(&mut self, id: u64, property: &str) -> Result<()> {
        let request_id = self.next_request_id();
        self.send_json(json!({
            "command": ["observe_property", id, property],
            "request_id": request_id,
        }))
    }

    pub fn send_command(&mut self, command: PlayerCommand) -> Result<()> {
        match command {
            PlayerCommand::LoadTrack {
                url,
                title,
                authorization,
            } => {
                let mut options = serde_json::Map::new();
                options.insert("force-media-title".to_string(), Value::String(title));
                if let Some(authorization) = authorization {
                    options.insert(
                        "http-header-fields".to_string(),
                        Value::String(format!("Authorization: Bearer {authorization}")),
                    );
                }

                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["loadfile", url, "replace", -1, Value::Object(options)],
                    "request_id": request_id,
                }))?;
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["set_property", "pause", false],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::Play => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["set_property", "pause", false],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::Pause => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["set_property", "pause", true],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::TogglePause => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["cycle", "pause"],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::Stop => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["stop"],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::SeekRelative { seconds } => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["seek", seconds, "relative"],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::SeekAbsolute { seconds } => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["seek", seconds, "absolute"],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::SetVolume { percent } => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["set_property", "volume", percent],
                    "request_id": request_id,
                }))
            }
            PlayerCommand::Shutdown => {
                let request_id = self.next_request_id();
                self.send_json(json!({
                    "command": ["quit"],
                    "request_id": request_id,
                }))
            }
        }
    }

    pub fn poll_message(&mut self) -> Result<Option<IpcMessage>> {
        #[cfg(windows)]
        {
            return self.connection.poll_message();
        }

        #[cfg(not(windows))]
        loop {
            if let Some(message) = drain_buffered_message(&mut self.read_buffer)? {
                return Ok(Some(message));
            }

            let mut scratch = [0_u8; 4096];
            match self.connection.read(&mut scratch) {
                Ok(0) => {
                    if self.read_buffer.is_empty() {
                        return Ok(Some(IpcMessage::Closed));
                    }
                    bail!("mpv IPC connection closed before a full JSON message was received");
                }
                Ok(bytes_read) => {
                    self.read_buffer.extend_from_slice(&scratch[..bytes_read]);
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(None),
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn send_json(&mut self, value: Value) -> Result<()> {
        let payload = serde_json::to_vec(&value)?;
        self.connection.write_all(&payload)?;
        self.connection.write_all(b"\n")?;
        Ok(())
    }

    fn next_request_id(&mut self) -> u64 {
        let request_id = self.request_id;
        self.request_id = self.request_id.saturating_add(1);
        request_id
    }
}

impl IpcConnection {
    #[cfg(unix)]
    fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path).with_context(|| {
            format!(
                "could not connect to mpv IPC socket at {}",
                socket_path.display()
            )
        })?;
        stream.set_nonblocking(true)?;
        Ok(Self::Unix(stream))
    }

    #[cfg(windows)]
    fn connect(socket_path: &Path) -> Result<Self> {
        let stream = DuplexPipeStream::<pipe_mode::Bytes>::connect_by_path(socket_path.as_os_str())
            .with_context(|| {
                format!(
                    "could not connect to mpv IPC pipe at {}",
                    socket_path.display()
                )
            })?;
        let (reader, writer) = stream.split();
        Ok(Self::Windows(WindowsPipeConnection {
            writer,
            reader_rx: spawn_windows_reader(reader),
        }))
    }

    #[cfg(not(any(unix, windows)))]
    fn connect(_socket_path: &Path) -> Result<Self> {
        bail!("mpv IPC is unsupported on this platform")
    }
}

#[cfg(windows)]
impl IpcConnection {
    fn poll_message(&mut self) -> Result<Option<IpcMessage>> {
        match self {
            Self::Windows(connection) => match connection.reader_rx.try_recv() {
                Ok(Ok(message)) => Ok(Some(message)),
                Ok(Err(error)) => bail!(error),
                Err(TryRecvError::Empty) => Ok(None),
                Err(TryRecvError::Disconnected) => Ok(Some(IpcMessage::Closed)),
            },
        }
    }
}

impl Read for IpcConnection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.read(buf),
            #[cfg(windows)]
            Self::Windows(_) => unreachable!("Windows IPC reads are handled by a reader thread"),
        }
    }
}

impl Write for IpcConnection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.write(buf),
            #[cfg(windows)]
            Self::Windows(connection) => connection.writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.flush(),
            #[cfg(windows)]
            Self::Windows(connection) => connection.writer.flush(),
        }
    }
}

fn drain_buffered_message(read_buffer: &mut Vec<u8>) -> Result<Option<IpcMessage>> {
    loop {
        let Some(newline) = read_buffer.iter().position(|byte| *byte == b'\n') else {
            return Ok(None);
        };

        let line = read_buffer.drain(..=newline).collect::<Vec<_>>();
        let text = String::from_utf8(line)?.trim().to_string();
        if text.is_empty() {
            continue;
        }

        return Ok(Some(parse_message(&text)?));
    }
}

#[cfg(windows)]
fn spawn_windows_reader(
    mut reader: RecvPipeStream<pipe_mode::Bytes>,
) -> Receiver<std::result::Result<IpcMessage, String>> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut read_buffer = Vec::new();
        let mut read_trace_budget = 12usize;

        loop {
            let mut scratch = [0_u8; 4096];
            let trace_read = take_read_trace(&mut read_trace_budget);
            if trace_read {
                info!(
                    "reading mpv IPC bytes on Windows: buffered_bytes={}",
                    read_buffer.len()
                );
            }

            match reader.read(&mut scratch) {
                Ok(0) => {
                    if trace_read {
                        info!("mpv IPC read returned EOF");
                    }
                    let message = if read_buffer.is_empty() {
                        Ok(IpcMessage::Closed)
                    } else {
                        Err(
                            "mpv IPC connection closed before a full JSON message was received"
                                .to_string(),
                        )
                    };
                    let _ = sender.send(message);
                    break;
                }
                Ok(bytes_read) => {
                    if trace_read {
                        info!("mpv IPC read returned {bytes_read} bytes");
                    }
                    read_buffer.extend_from_slice(&scratch[..bytes_read]);
                    if !forward_buffered_messages(&mut read_buffer, &sender) {
                        break;
                    }
                }
                Err(error) => {
                    if trace_read {
                        info!("mpv IPC read failed: {error}");
                    }
                    let _ = sender.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });

    receiver
}

#[cfg(windows)]
fn forward_buffered_messages(
    read_buffer: &mut Vec<u8>,
    sender: &mpsc::Sender<std::result::Result<IpcMessage, String>>,
) -> bool {
    loop {
        match drain_buffered_message(read_buffer) {
            Ok(Some(message)) => {
                if sender.send(Ok(message)).is_err() {
                    return false;
                }
            }
            Ok(None) => return true,
            Err(error) => {
                let _ = sender.send(Err(error.to_string()));
                return false;
            }
        }
    }
}

#[cfg(windows)]
fn take_read_trace(remaining: &mut usize) -> bool {
    if *remaining == 0 {
        return false;
    }

    *remaining = (*remaining).saturating_sub(1);
    true
}

pub fn parse_message(text: &str) -> Result<IpcMessage> {
    let message: RawIpcMessage =
        serde_json::from_str(text).with_context(|| format!("invalid mpv IPC payload: {text}"))?;

    if let Some(event) = message.event.as_deref() {
        return Ok(IpcMessage::Event(match event {
            "playback-restart" => IpcEvent::PlaybackRestart,
            "end-file" => IpcEvent::EndFile {
                reason: message.reason,
            },
            "property-change" => IpcEvent::PropertyChange {
                name: message.name.unwrap_or_default(),
                data: message.data,
            },
            other => IpcEvent::PropertyChange {
                name: other.to_string(),
                data: message.data,
            },
        }));
    }

    Ok(IpcMessage::Response(IpcResponse {
        request_id: message.request_id,
        error: message.error,
    }))
}

impl IpcEvent {
    pub fn into_player_event(self) -> Option<PlayerEvent> {
        match self {
            IpcEvent::PlaybackRestart => Some(PlayerEvent::PlaybackStarted),
            IpcEvent::EndFile { reason } => match reason.as_deref() {
                Some("eof") => Some(PlayerEvent::TrackEnded),
                _ => Some(PlayerEvent::PlaybackStopped),
            },
            IpcEvent::PropertyChange { name, data } => match name.as_str() {
                "pause" => match data.and_then(|value| value.as_bool()) {
                    Some(true) => Some(PlayerEvent::PlaybackPaused),
                    Some(false) => Some(PlayerEvent::PlaybackResumed),
                    None => None,
                },
                "playback-time" => data
                    .and_then(|value| value.as_f64())
                    .map(|seconds| PlayerEvent::PositionChanged { seconds }),
                "duration" => Some(PlayerEvent::DurationChanged {
                    seconds: data.and_then(|value| value.as_f64()),
                }),
                "volume" => data
                    .and_then(|value| value.as_f64())
                    .map(|percent| PlayerEvent::VolumeChanged { percent }),
                _ => None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_property_change_messages() {
        let message =
            parse_message(r#"{"event":"property-change","name":"playback-time","data":12.5}"#)
                .expect("message should parse");

        assert_eq!(
            message,
            IpcMessage::Event(IpcEvent::PropertyChange {
                name: "playback-time".to_string(),
                data: Some(json!(12.5)),
            })
        );
    }

    #[test]
    fn maps_end_file_to_track_end() {
        let message =
            parse_message(r#"{"event":"end-file","reason":"eof"}"#).expect("message should parse");

        let event = match message {
            IpcMessage::Event(event) => event,
            other => panic!("expected event, got {other:?}"),
        };

        assert_eq!(event.into_player_event(), Some(PlayerEvent::TrackEnded));
    }
}
