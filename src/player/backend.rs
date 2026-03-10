use anyhow::Result;

use crate::player::{command::PlayerCommand, event::PlayerEvent};

pub trait PlayerBackend {
    fn send(&mut self, command: PlayerCommand) -> Result<()>;
    fn poll_event(&mut self) -> Result<Option<PlayerEvent>>;
}
