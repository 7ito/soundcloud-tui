use std::fs;

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{config::paths::AppPaths, soundcloud::models::TrackSummary};

const MAX_RECENTLY_PLAYED: usize = 100;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct RecentlyPlayedEntry {
    pub track: TrackSummary,
    pub context: String,
    pub played_at_epoch: i64,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecentlyPlayedStore {
    pub entries: Vec<RecentlyPlayedEntry>,
}

impl RecentlyPlayedStore {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.history_file.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&paths.history_file)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&paths.history_file, raw)?;
        Ok(())
    }

    pub fn record(&mut self, track: TrackSummary, context: String) {
        self.entries.retain(|entry| entry.track.urn != track.urn);
        self.entries.insert(
            0,
            RecentlyPlayedEntry {
                track,
                context,
                played_at_epoch: Utc::now().timestamp(),
            },
        );
        self.entries.truncate(MAX_RECENTLY_PLAYED);
    }
}

#[cfg(test)]
mod tests {
    use super::RecentlyPlayedStore;
    use crate::soundcloud::models::TrackSummary;

    #[test]
    fn record_keeps_latest_entry_per_track() {
        let mut store = RecentlyPlayedStore::default();
        let track = TrackSummary {
            urn: "soundcloud:tracks:1".to_string(),
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            artist_urn: None,
            duration_ms: Some(180_000),
            permalink_url: None,
            artwork_url: None,
            access: None,
            streamable: true,
        };

        store.record(track.clone(), "Feed".to_string());
        store.record(track, "Liked Songs".to_string());

        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries[0].context, "Liked Songs");
    }
}
