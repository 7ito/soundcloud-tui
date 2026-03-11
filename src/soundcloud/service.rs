use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::soundcloud::{
    client::SoundcloudClient,
    models::{
        FeedItem, FeedOrigin, PlaylistSummary, SearchResults, TrackAccess, TrackSummary,
        UserSummary,
    },
    paging::Page,
};

const ACCESS_ALL: &str = "playable,preview,blocked";
const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ResolvedStream {
    pub url: String,
    pub preview: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PlaylistTrackAddResult {
    Added,
    AlreadyPresent,
}

#[derive(Debug, Clone)]
pub struct SoundcloudService {
    client: SoundcloudClient,
}

#[derive(Debug, Deserialize)]
struct ApiPage<T> {
    collection: Vec<T>,
    next_href: Option<String>,
}

impl<T> ApiPage<T> {
    fn into_page<U>(self, mut map: impl FnMut(T) -> U) -> Page<U> {
        Page {
            items: self.collection.into_iter().map(&mut map).collect(),
            next_href: self.next_href,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiTrack {
    urn: String,
    title: String,
    duration: Option<u64>,
    permalink_url: Option<String>,
    artwork_url: Option<String>,
    access: Option<String>,
    streamable: Option<bool>,
    user: Option<ApiUser>,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylist {
    urn: String,
    title: String,
    description: Option<String>,
    track_count: Option<usize>,
    duration: Option<u64>,
    permalink_url: Option<String>,
    artwork_url: Option<String>,
    playlist_type: Option<String>,
    release_year: Option<i32>,
    user: Option<ApiUser>,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    urn: String,
    username: String,
    permalink_url: Option<String>,
    avatar_url: Option<String>,
    followers_count: Option<u64>,
    track_count: Option<u64>,
    playlist_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApiStreams {
    hls_aac_160_url: Option<String>,
    http_mp3_128_url: Option<String>,
    hls_mp3_128_url: Option<String>,
    preview_mp3_128_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylistDetail {
    tracks: Vec<ApiPlaylistTrackRef>,
}

#[derive(Debug, Deserialize)]
struct ApiPlaylistTrackRef {
    id: Option<Value>,
    urn: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistRequestById {
    playlist: UpdatePlaylistTracksById,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistTracksById {
    tracks: Vec<UpdatePlaylistTrackById>,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistTrackById {
    id: String,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistRequestByUrn {
    playlist: UpdatePlaylistTracksByUrn,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistTracksByUrn {
    tracks: Vec<UpdatePlaylistTrackByUrn>,
}

#[derive(Debug, Serialize)]
struct UpdatePlaylistTrackByUrn {
    urn: String,
}

#[derive(Debug, Deserialize)]
struct ApiActivity {
    #[serde(rename = "type")]
    activity_type: String,
    created_at: Option<String>,
    origin: ApiFeedOrigin,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ApiFeedOrigin {
    Track(ApiTrack),
    Playlist(ApiPlaylist),
}

impl SoundcloudService {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: SoundcloudClient::new()?,
        })
    }

    pub async fn load_feed(
        &self,
        access_token: &str,
        next_href: Option<&str>,
    ) -> Result<Page<FeedItem>> {
        let page: ApiPage<ApiActivity> = match next_href {
            Some(next_href) => self.client.get_by_href(next_href, access_token).await?,
            None => {
                self.client
                    .get(
                        "/me/feed/tracks",
                        access_token,
                        &[
                            ("limit", PAGE_SIZE.to_string()),
                            ("access", ACCESS_ALL.to_string()),
                        ],
                    )
                    .await?
            }
        };

        Ok(page.into_page(normalize_activity))
    }

    pub async fn load_liked_tracks(
        &self,
        access_token: &str,
        next_href: Option<&str>,
    ) -> Result<Page<TrackSummary>> {
        self.fetch_track_page(
            access_token,
            next_href,
            "/me/likes/tracks",
            &[
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("access", ACCESS_ALL.to_string()),
            ],
        )
        .await
    }

    pub async fn load_followings(
        &self,
        access_token: &str,
        next_href: Option<&str>,
    ) -> Result<Page<UserSummary>> {
        let page: ApiPage<ApiUser> = match next_href {
            Some(next_href) => self.client.get_by_href(next_href, access_token).await?,
            None => {
                self.client
                    .get(
                        "/me/followings",
                        access_token,
                        &[("limit", PAGE_SIZE.to_string())],
                    )
                    .await?
            }
        };

        Ok(page.into_page(normalize_user))
    }

    pub async fn load_playlists(
        &self,
        access_token: &str,
        next_href: Option<&str>,
    ) -> Result<Page<PlaylistSummary>> {
        self.fetch_playlist_page(
            access_token,
            next_href,
            "/me/playlists",
            &[
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("show_tracks", "false".to_string()),
            ],
        )
        .await
    }

    pub async fn load_albums(
        &self,
        access_token: &str,
        next_href: Option<&str>,
    ) -> Result<Page<PlaylistSummary>> {
        let page = self
            .fetch_playlist_page(
                access_token,
                next_href,
                "/me/likes/playlists",
                &[
                    ("limit", PAGE_SIZE.to_string()),
                    ("linked_partitioning", "true".to_string()),
                ],
            )
            .await?;

        Ok(Page {
            items: page
                .items
                .into_iter()
                .filter(PlaylistSummary::looks_like_album)
                .collect(),
            next_href: page.next_href,
        })
    }

    pub async fn load_playlist_tracks(
        &self,
        access_token: &str,
        playlist_urn: &str,
        next_href: Option<&str>,
    ) -> Result<Page<TrackSummary>> {
        let path = format!("/playlists/{playlist_urn}/tracks");

        self.fetch_track_page(
            access_token,
            next_href,
            path.as_str(),
            &[
                ("linked_partitioning", "true".to_string()),
                ("access", ACCESS_ALL.to_string()),
            ],
        )
        .await
    }

    pub async fn load_user_tracks(
        &self,
        access_token: &str,
        user_urn: &str,
        next_href: Option<&str>,
    ) -> Result<Page<TrackSummary>> {
        let path = format!("/users/{user_urn}/tracks");

        self.fetch_track_page(
            access_token,
            next_href,
            path.as_str(),
            &[
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("access", ACCESS_ALL.to_string()),
            ],
        )
        .await
    }

    pub async fn load_user_playlists(
        &self,
        access_token: &str,
        user_urn: &str,
        next_href: Option<&str>,
    ) -> Result<Page<PlaylistSummary>> {
        let path = format!("/users/{user_urn}/playlists");

        self.fetch_playlist_page(
            access_token,
            next_href,
            path.as_str(),
            &[
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("show_tracks", "false".to_string()),
            ],
        )
        .await
    }

    pub async fn search_all(&self, access_token: &str, query: &str) -> Result<SearchResults> {
        let (tracks, playlists, users) = tokio::try_join!(
            self.search_tracks(access_token, query, None),
            self.search_playlists(access_token, query, None),
            self.search_users(access_token, query, None),
        )?;

        Ok(SearchResults {
            tracks,
            playlists,
            users,
        })
    }

    pub async fn search_tracks(
        &self,
        access_token: &str,
        query: &str,
        next_href: Option<&str>,
    ) -> Result<Page<TrackSummary>> {
        self.fetch_track_page(
            access_token,
            next_href,
            "/tracks",
            &[
                ("q", query.to_string()),
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("access", ACCESS_ALL.to_string()),
            ],
        )
        .await
    }

    pub async fn search_playlists(
        &self,
        access_token: &str,
        query: &str,
        next_href: Option<&str>,
    ) -> Result<Page<PlaylistSummary>> {
        self.fetch_playlist_page(
            access_token,
            next_href,
            "/playlists",
            &[
                ("q", query.to_string()),
                ("limit", PAGE_SIZE.to_string()),
                ("linked_partitioning", "true".to_string()),
                ("show_tracks", "false".to_string()),
                ("access", ACCESS_ALL.to_string()),
            ],
        )
        .await
    }

    pub async fn search_users(
        &self,
        access_token: &str,
        query: &str,
        next_href: Option<&str>,
    ) -> Result<Page<UserSummary>> {
        let page: ApiPage<ApiUser> = match next_href {
            Some(next_href) => self.client.get_by_href(next_href, access_token).await?,
            None => {
                self.client
                    .get(
                        "/users",
                        access_token,
                        &[
                            ("q", query.to_string()),
                            ("limit", PAGE_SIZE.to_string()),
                            ("linked_partitioning", "true".to_string()),
                        ],
                    )
                    .await?
            }
        };

        Ok(page.into_page(normalize_user))
    }

    pub async fn resolve_stream(
        &self,
        access_token: &str,
        track: &TrackSummary,
    ) -> Result<ResolvedStream> {
        let path = format!("/tracks/{}/streams", track.urn);
        let streams: ApiStreams = self.client.get(&path, access_token, &[]).await?;

        if let Some(url) = streams.hls_aac_160_url.or(streams.hls_mp3_128_url) {
            return Ok(ResolvedStream {
                url,
                preview: false,
            });
        }

        if let Some(url) = streams.http_mp3_128_url {
            return Ok(ResolvedStream {
                url,
                preview: false,
            });
        }

        if let Some(url) = streams.preview_mp3_128_url {
            return Ok(ResolvedStream { url, preview: true });
        }

        anyhow::bail!(
            "SoundCloud did not return a playable stream URL for {}",
            track.title
        )
    }

    pub async fn like_track(&self, access_token: &str, track: &TrackSummary) -> Result<()> {
        let track_id = soundcloud_identifier_suffix(&track.urn)?;
        let path = format!("/likes/tracks/{track_id}");
        self.client.post_empty(&path, access_token).await
    }

    pub async fn add_track_to_playlist(
        &self,
        access_token: &str,
        playlist: &PlaylistSummary,
        track: &TrackSummary,
    ) -> Result<PlaylistTrackAddResult> {
        let playlist_id = soundcloud_identifier_suffix(&playlist.urn)?;
        let path = format!("/playlists/{playlist_id}");
        let detail: ApiPlaylistDetail = self
            .client
            .get(&path, access_token, &[("show_tracks", "true".to_string())])
            .await?;

        let mut track_urns = detail
            .tracks
            .into_iter()
            .map(api_playlist_track_urn)
            .collect::<Result<Vec<_>>>()?;

        if track_urns.contains(&track.urn) {
            return Ok(PlaylistTrackAddResult::AlreadyPresent);
        }

        track_urns.push(track.urn.clone());
        self.update_playlist_tracks(access_token, &path, &track_urns)
            .await?;
        Ok(PlaylistTrackAddResult::Added)
    }

    async fn update_playlist_tracks(
        &self,
        access_token: &str,
        path: &str,
        track_urns: &[String],
    ) -> Result<()> {
        let track_ids = track_urns
            .iter()
            .map(|urn| soundcloud_identifier_suffix(urn))
            .collect::<Result<Vec<_>>>()?;

        let id_body = UpdatePlaylistRequestById {
            playlist: UpdatePlaylistTracksById {
                tracks: track_ids
                    .iter()
                    .cloned()
                    .map(|id| UpdatePlaylistTrackById { id })
                    .collect(),
            },
        };

        if self
            .client
            .put_json(path, access_token, &id_body)
            .await
            .is_ok()
        {
            return Ok(());
        }

        let urn_body = UpdatePlaylistRequestByUrn {
            playlist: UpdatePlaylistTracksByUrn {
                tracks: track_urns
                    .iter()
                    .cloned()
                    .map(|urn| UpdatePlaylistTrackByUrn { urn })
                    .collect(),
            },
        };

        if self
            .client
            .put_json(path, access_token, &urn_body)
            .await
            .is_ok()
        {
            return Ok(());
        }

        let form_fields = track_ids
            .into_iter()
            .map(|id| ("playlist[tracks][][id]".to_string(), id))
            .collect::<Vec<_>>();

        self.client.put_form(path, access_token, &form_fields).await
    }

    async fn fetch_track_page(
        &self,
        access_token: &str,
        next_href: Option<&str>,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Page<TrackSummary>> {
        let page: ApiPage<ApiTrack> = match next_href {
            Some(next_href) => self.client.get_by_href(next_href, access_token).await?,
            None => self.client.get(path, access_token, query).await?,
        };

        Ok(page.into_page(normalize_track))
    }

    async fn fetch_playlist_page(
        &self,
        access_token: &str,
        next_href: Option<&str>,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Page<PlaylistSummary>> {
        let page: ApiPage<ApiPlaylist> = match next_href {
            Some(next_href) => self.client.get_by_href(next_href, access_token).await?,
            None => self.client.get(path, access_token, query).await?,
        };

        Ok(page.into_page(normalize_playlist))
    }
}

fn normalize_activity(activity: ApiActivity) -> FeedItem {
    let origin = match activity.origin {
        ApiFeedOrigin::Track(track) => FeedOrigin::Track(normalize_track(track)),
        ApiFeedOrigin::Playlist(playlist) => FeedOrigin::Playlist(normalize_playlist(playlist)),
    };

    FeedItem {
        activity_type: activity.activity_type,
        created_at: activity.created_at,
        origin,
    }
}

fn normalize_track(track: ApiTrack) -> TrackSummary {
    let (artist, artist_urn) = track
        .user
        .map(|user| (user.username, Some(user.urn)))
        .unwrap_or_else(|| ("Unknown artist".to_string(), None));

    TrackSummary {
        urn: track.urn,
        title: track.title,
        artist,
        artist_urn,
        duration_ms: track.duration,
        permalink_url: track.permalink_url,
        artwork_url: track.artwork_url,
        access: track.access.map(normalize_track_access),
        streamable: track.streamable.unwrap_or(false),
    }
}

fn normalize_playlist(playlist: ApiPlaylist) -> PlaylistSummary {
    let creator = playlist
        .user
        .as_ref()
        .map(|user| user.username.clone())
        .unwrap_or_else(|| "Unknown creator".to_string());

    PlaylistSummary {
        urn: playlist.urn,
        title: playlist.title,
        description: playlist.description.unwrap_or_default(),
        creator,
        creator_urn: playlist.user.map(|user| user.urn),
        track_count: playlist.track_count.unwrap_or(0),
        duration_ms: playlist.duration,
        permalink_url: playlist.permalink_url,
        artwork_url: playlist.artwork_url,
        playlist_type: playlist.playlist_type,
        release_year: playlist.release_year,
    }
}

fn normalize_user(user: ApiUser) -> UserSummary {
    UserSummary {
        urn: user.urn,
        username: user.username,
        permalink_url: user.permalink_url,
        avatar_url: user.avatar_url,
        followers_count: user.followers_count.unwrap_or(0),
        track_count: user.track_count.unwrap_or(0),
        playlist_count: user.playlist_count.unwrap_or(0),
    }
}

fn normalize_track_access(access: String) -> TrackAccess {
    match access.as_str() {
        "playable" => TrackAccess::Playable,
        "preview" => TrackAccess::Preview,
        "blocked" => TrackAccess::Blocked,
        _ => TrackAccess::Unknown(access),
    }
}

fn api_playlist_track_urn(track: ApiPlaylistTrackRef) -> Result<String> {
    track
        .urn
        .or_else(|| {
            track.id.and_then(|id| {
                soundcloud_identifier_from_value(&id)
                    .map(|identifier| format!("soundcloud:tracks:{identifier}"))
            })
        })
        .ok_or_else(|| anyhow!("SoundCloud playlist response did not include track identifiers"))
}

fn soundcloud_identifier_suffix(urn: &str) -> Result<String> {
    urn.rsplit(':')
        .next()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("SoundCloud urn '{urn}' is missing an identifier suffix"))
}

fn soundcloud_identifier_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_update_id_payload_uses_string_identifiers() {
        let payload = UpdatePlaylistRequestById {
            playlist: UpdatePlaylistTracksById {
                tracks: vec![
                    UpdatePlaylistTrackById {
                        id: "1234567890123".to_string(),
                    },
                    UpdatePlaylistTrackById {
                        id: "42".to_string(),
                    },
                ],
            },
        };

        let value = serde_json::to_value(payload).expect("payload should serialize");
        assert_eq!(value["playlist"]["tracks"][0]["id"], "1234567890123");
        assert_eq!(value["playlist"]["tracks"][1]["id"], "42");
    }

    #[test]
    fn playlist_track_urn_falls_back_to_id_value() {
        let track = ApiPlaylistTrackRef {
            id: Some(Value::String("1234567890123".to_string())),
            urn: None,
        };

        assert_eq!(
            api_playlist_track_urn(track).expect("urn should be derived from id"),
            "soundcloud:tracks:1234567890123"
        );
    }

    #[test]
    fn identifier_suffix_returns_string_without_numeric_parsing() {
        assert_eq!(
            soundcloud_identifier_suffix("soundcloud:tracks:1234567890123")
                .expect("suffix should parse"),
            "1234567890123"
        );
    }
}
