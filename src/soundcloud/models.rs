use serde::{Deserialize, Serialize};

use crate::util::time::format_seconds;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TrackAccess {
    Playable,
    Preview,
    Blocked,
    Unknown(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrackSummary {
    pub urn: String,
    pub title: String,
    pub artist: String,
    pub artist_urn: Option<String>,
    pub duration_ms: Option<u64>,
    pub permalink_url: Option<String>,
    pub artwork_url: Option<String>,
    pub access: Option<TrackAccess>,
    pub streamable: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PlaylistSummary {
    pub urn: String,
    pub title: String,
    pub description: String,
    pub creator: String,
    pub creator_urn: Option<String>,
    pub track_count: usize,
    pub duration_ms: Option<u64>,
    pub permalink_url: Option<String>,
    pub artwork_url: Option<String>,
    pub playlist_type: Option<String>,
    pub release_year: Option<i32>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UserSummary {
    pub urn: String,
    pub username: String,
    pub permalink_url: Option<String>,
    pub avatar_url: Option<String>,
    pub followers_count: u64,
    pub track_count: u64,
    pub playlist_count: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FeedOrigin {
    Track(TrackSummary),
    Playlist(PlaylistSummary),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FeedItem {
    pub activity_type: String,
    pub created_at: Option<String>,
    pub origin: FeedOrigin,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SearchResults {
    pub tracks: crate::soundcloud::paging::Page<TrackSummary>,
    pub playlists: crate::soundcloud::paging::Page<PlaylistSummary>,
    pub users: crate::soundcloud::paging::Page<UserSummary>,
}

impl TrackSummary {
    pub fn can_attempt_playback(&self) -> bool {
        !matches!(self.access, Some(TrackAccess::Blocked))
    }

    pub fn duration_label(&self) -> String {
        self.duration_ms
            .map(|duration| format_seconds(duration / 1000))
            .unwrap_or_else(|| "--:--".to_string())
    }

    pub fn access_label(&self) -> &'static str {
        match self.access.as_ref() {
            Some(TrackAccess::Playable) => "Playable",
            Some(TrackAccess::Preview) => "Preview",
            Some(TrackAccess::Blocked) => "Blocked",
            Some(TrackAccess::Unknown(_)) | None => "Unknown",
        }
    }
}

impl PlaylistSummary {
    pub fn track_count_label(&self) -> String {
        format!("{} tracks", self.track_count)
    }

    pub fn year_label(&self) -> String {
        self.release_year
            .map(|year| year.to_string())
            .unwrap_or_else(|| "--".to_string())
    }

    pub fn looks_like_album(&self) -> bool {
        matches!(self.playlist_type.as_deref(), Some("album")) || self.release_year.is_some()
    }
}

impl UserSummary {
    pub fn followers_label(&self) -> String {
        abbreviate_count(self.followers_count)
    }

    pub fn spotlight_label(&self) -> String {
        format!(
            "{} tracks | {} playlists",
            self.track_count, self.playlist_count
        )
    }
}

pub fn abbreviate_count(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=999_999 => format!("{:.1}K", count as f64 / 1_000.0)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
        1_000_000..=999_999_999 => format!("{:.1}M", count as f64 / 1_000_000.0)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
        _ => format!("{:.1}B", count as f64 / 1_000_000_000.0)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    }
}
