use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use anyhow::Result;
use tokio::sync::mpsc as tokio_mpsc;

use crate::{
    app::AppEvent,
    visualizer::{SpectrumFrame, cpal_capture::CpalCapture},
};

#[cfg(target_os = "linux")]
use crate::visualizer::pipewire_capture::PipeWireCapture;

const FRAME_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum VisualizerCommand {
    Start,
    Stop,
    Shutdown,
}

#[derive(Clone)]
pub struct VisualizerHandle {
    command_tx: mpsc::Sender<VisualizerCommand>,
}

impl VisualizerHandle {
    pub fn spawn(app_events: tokio_mpsc::UnboundedSender<AppEvent>) -> Self {
        let (command_tx, command_rx) = mpsc::channel();

        thread::spawn(move || {
            let mut capture: Option<CaptureBackend> = None;
            let mut emitting = false;

            loop {
                match command_rx.recv_timeout(FRAME_INTERVAL) {
                    Ok(VisualizerCommand::Start) => {
                        if capture.is_some() {
                            emitting = true;
                            continue;
                        }

                        match open_capture_backend() {
                            Ok(new_capture) => {
                                let _ =
                                    app_events.send(AppEvent::VisualizerCaptureStarted(format!(
                                        "Visualizer capture ready on {}.",
                                        new_capture.device_name()
                                    )));
                                emitting = true;
                                capture = Some(new_capture);
                            }
                            Err(error) => {
                                emitting = false;
                                let _ = app_events.send(AppEvent::VisualizerCaptureFailed(error));
                            }
                        }
                    }
                    Ok(VisualizerCommand::Stop) => {
                        emitting = false;
                    }
                    Ok(VisualizerCommand::Shutdown) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                let Some(active_capture) = capture.as_ref() else {
                    continue;
                };
                if !emitting {
                    continue;
                }

                if !active_capture.is_active() {
                    if let Some(error) = active_capture.take_error() {
                        let _ = app_events.send(AppEvent::VisualizerCaptureFailed(error));
                    }
                    emitting = false;
                    capture = None;
                    continue;
                }

                match active_capture.frame() {
                    Ok(frame) => {
                        let _ = app_events.send(AppEvent::VisualizerFrame(frame));
                    }
                    Err(error) => {
                        let _ = app_events.send(AppEvent::VisualizerCaptureFailed(error));
                        capture = None;
                    }
                }
            }
        });

        Self { command_tx }
    }

    pub fn send(&self, command: VisualizerCommand) -> Result<()> {
        self.command_tx.send(command)?;
        Ok(())
    }
}

enum CaptureBackend {
    Cpal(CpalCapture),
    #[cfg(target_os = "linux")]
    PipeWire(PipeWireCapture),
}

impl CaptureBackend {
    fn frame(&self) -> Result<SpectrumFrame, String> {
        match self {
            Self::Cpal(capture) => capture.frame(),
            #[cfg(target_os = "linux")]
            Self::PipeWire(capture) => capture.frame(),
        }
    }

    fn device_name(&self) -> &str {
        match self {
            Self::Cpal(capture) => capture.device_name(),
            #[cfg(target_os = "linux")]
            Self::PipeWire(capture) => capture.device_name(),
        }
    }

    fn is_active(&self) -> bool {
        match self {
            Self::Cpal(capture) => capture.is_active(),
            #[cfg(target_os = "linux")]
            Self::PipeWire(capture) => capture.is_active(),
        }
    }

    fn take_error(&self) -> Option<String> {
        match self {
            Self::Cpal(capture) => capture.take_error(),
            #[cfg(target_os = "linux")]
            Self::PipeWire(capture) => capture.take_error(),
        }
    }
}

fn open_capture_backend() -> Result<CaptureBackend, String> {
    #[cfg(target_os = "linux")]
    {
        match PipeWireCapture::open() {
            Ok(capture) => return Ok(CaptureBackend::PipeWire(capture)),
            Err(pipewire_error) => match CpalCapture::open() {
                Ok(capture) => return Ok(CaptureBackend::Cpal(capture)),
                Err(cpal_error) => {
                    return Err(format!(
                        "PipeWire capture failed: {pipewire_error}. CPAL fallback failed: {cpal_error}"
                    ));
                }
            },
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        CpalCapture::open().map(CaptureBackend::Cpal)
    }
}
