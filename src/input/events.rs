use std::{
    thread,
    time::{Duration, Instant},
};

use crossterm::event::{
    self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use log::warn;
use tokio::sync::mpsc;

use crate::{
    app::{Action, AppEvent},
    input::keys,
};

pub struct EventHandler {
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    _input_task: thread::JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let input_sender = sender.clone();

        let input_task = thread::spawn(move || {
            let mut last_tick = Instant::now();

            loop {
                let timeout = tick_rate.saturating_sub(last_tick.elapsed());

                match event::poll(timeout) {
                    Ok(true) => match event::read() {
                        Ok(CrosstermEvent::Key(key)) if key.kind == KeyEventKind::Press => {
                            if input_sender.send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            if input_sender.send(AppEvent::Mouse(mouse)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Paste(text)) => {
                            if input_sender.send(AppEvent::Paste(text)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            if input_sender
                                .send(AppEvent::Resize { width, height })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(error) => {
                            warn!("input event loop exiting after read error: {error}");
                            break;
                        }
                    },
                    Ok(false) => {}
                    Err(error) => {
                        warn!("input event loop exiting after poll error: {error}");
                        break;
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if input_sender.send(AppEvent::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self {
            receiver,
            _input_task: input_task,
        }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}

pub fn map_main_key_event(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(keys::QUIT_KEY) => Some(Action::Quit),
        KeyCode::Tab => Some(Action::FocusNext),
        KeyCode::BackTab => Some(Action::FocusPrevious),
        KeyCode::Up | KeyCode::Char(keys::MOVE_UP_KEY) => Some(Action::MoveUp),
        KeyCode::Down | KeyCode::Char(keys::MOVE_DOWN_KEY) => Some(Action::MoveDown),
        KeyCode::Enter => Some(Action::Select),
        _ => None,
    }
}

pub fn is_global_quit_key(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_main_navigation_keys() {
        assert_eq!(
            map_main_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Some(Action::Quit)
        );
        assert_eq!(
            map_main_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(Action::MoveDown)
        );
    }

    #[test]
    fn ctrl_c_is_global_quit_key() {
        assert!(is_global_quit_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
    }
}
