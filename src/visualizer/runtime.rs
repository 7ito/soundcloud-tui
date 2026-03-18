use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use anyhow::Result;
use tokio::sync::mpsc as tokio_mpsc;

use crate::{
    app::AppEvent,
    visualizer::{SpectrumFrame, VisualizerTap},
};

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
    pub fn spawn(app_events: tokio_mpsc::UnboundedSender<AppEvent>, tap: VisualizerTap) -> Self {
        let (command_tx, command_rx) = mpsc::channel();

        thread::spawn(move || {
            let mut emitting = false;

            loop {
                match command_rx.recv_timeout(FRAME_INTERVAL) {
                    Ok(VisualizerCommand::Start) => {
                        emitting = true;
                        let _ = app_events.send(AppEvent::VisualizerCaptureStarted(
                            "Visualizer synced to playback stream.".to_string(),
                        ));
                    }
                    Ok(VisualizerCommand::Stop) => {
                        emitting = false;
                    }
                    Ok(VisualizerCommand::Shutdown) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                if !emitting {
                    continue;
                }

                match tap.current_frame() {
                    Ok(frame) => {
                        let _ = app_events.send(AppEvent::VisualizerFrame(frame));
                    }
                    Err(error) => {
                        emitting = false;
                        let _ = app_events.send(AppEvent::VisualizerCaptureFailed(error));
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

#[allow(dead_code)]
fn _default_frame() -> SpectrumFrame {
    SpectrumFrame::default()
}
