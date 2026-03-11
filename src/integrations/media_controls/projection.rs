use std::time::Duration;

use crate::{
    app::{AppState, RepeatMode, state::PlaybackStatus},
    soundcloud::models::TrackSummary,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MediaControlsState {
    pub track: Option<MediaControlsTrack>,
    pub playback: MediaPlaybackState,
    pub position: Option<Duration>,
    pub volume_percent: f64,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
    pub can_go_next: bool,
    pub can_go_previous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaControlsTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<Duration>,
    pub permalink_url: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPlaybackState {
    Stopped,
    Paused,
    Playing,
}

impl MediaControlsState {
    pub fn from_app(app: &AppState) -> Self {
        let track = app
            .now_playing
            .track
            .as_ref()
            .map(MediaControlsTrack::from_track);
        let position = track
            .as_ref()
            .map(|_| seconds_to_duration(app.player.position_seconds));

        Self {
            track,
            playback: MediaPlaybackState::from_status(app.player.status),
            position,
            volume_percent: app.player.volume_percent,
            shuffle_enabled: app.player.shuffle_enabled,
            repeat_mode: app.player.repeat_mode,
            can_go_next: app.can_play_next_track(),
            can_go_previous: app.can_play_previous_track(),
        }
    }

    pub fn can_play(&self) -> bool {
        self.track.is_some()
    }

    pub fn can_pause(&self) -> bool {
        self.track.is_some()
    }

    pub fn metadata_matches(&self, other: &Self) -> bool {
        self.track == other.track
    }

    pub fn playback_matches(&self, other: &Self) -> bool {
        self.playback == other.playback && self.position == other.position
    }
}

impl MediaControlsTrack {
    fn from_track(track: &TrackSummary) -> Self {
        Self {
            id: track.urn.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            duration: track.duration_ms.map(Duration::from_millis),
            permalink_url: track.permalink_url.clone(),
            artwork_url: track.artwork_url.clone(),
        }
    }
}

impl MediaPlaybackState {
    fn from_status(status: PlaybackStatus) -> Self {
        match status {
            PlaybackStatus::Stopped => Self::Stopped,
            PlaybackStatus::Paused => Self::Paused,
            PlaybackStatus::Playing | PlaybackStatus::Buffering => Self::Playing,
        }
    }
}

fn seconds_to_duration(seconds: f64) -> Duration {
    Duration::from_secs_f64(seconds.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{AppState, RepeatMode},
        soundcloud::models::{TrackAccess, TrackSummary},
    };

    #[test]
    fn projection_handles_missing_track() {
        let app = AppState::new();
        let state = MediaControlsState::from_app(&app);

        assert!(state.track.is_none());
        assert_eq!(state.playback, MediaPlaybackState::Stopped);
        assert!(!state.can_play());
        assert!(!state.can_pause());
    }

    #[test]
    fn projection_clones_track_state() {
        let mut app = AppState::new();
        let track = TrackSummary {
            urn: "soundcloud:tracks:42".to_string(),
            title: "Signal".to_string(),
            artist: "Four Tet".to_string(),
            artist_urn: Some("soundcloud:users:7".to_string()),
            duration_ms: Some(245_000),
            permalink_url: Some("https://soundcloud.com/four-tet/signal".to_string()),
            artwork_url: Some("https://i1.sndcdn.com/artworks-signal.jpg".to_string()),
            access: Some(TrackAccess::Playable),
            streamable: true,
        };

        app.now_playing.track = Some(track);
        app.player.status = PlaybackStatus::Paused;
        app.player.position_seconds = 12.5;
        app.player.volume_percent = 65.0;
        app.player.shuffle_enabled = true;
        app.player.repeat_mode = RepeatMode::Queue;

        let state = MediaControlsState::from_app(&app);
        let track = state.track.as_ref().expect("track projection");

        assert_eq!(track.id, "soundcloud:tracks:42");
        assert_eq!(track.title, "Signal");
        assert_eq!(track.artist, "Four Tet");
        assert_eq!(track.duration, Some(Duration::from_millis(245_000)));
        assert_eq!(state.playback, MediaPlaybackState::Paused);
        assert_eq!(state.position, Some(Duration::from_secs_f64(12.5)));
        assert_eq!(state.volume_percent, 65.0);
        assert!(state.shuffle_enabled);
        assert_eq!(state.repeat_mode, RepeatMode::Queue);
        assert!(state.can_play());
        assert!(state.can_pause());
    }
}
