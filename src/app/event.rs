use crossterm::event::KeyEvent;

use crate::{
    app::PlaybackIntent,
    player::event::PlayerEvent,
    soundcloud::{
        auth::{AuthorizationRequest, AuthorizedSession},
        models::{PlaylistSummary, SearchResults, TrackSummary, UserSummary},
        paging::Page,
    },
};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Paste(String),
    Tick,
    Resize {
        width: u16,
        height: u16,
    },
    CredentialsSaved(AuthorizationRequest),
    CredentialsSaveFailed(String),
    AuthRestoreComplete(Result<AuthorizedSession, String>),
    AuthCallbackCaptured(String),
    AuthCallbackFailed(String),
    AuthCompleted(Result<AuthorizedSession, String>),
    FeedLoaded {
        session: AuthorizedSession,
        page: Page<crate::soundcloud::models::FeedItem>,
        append: bool,
    },
    FeedFailed(String),
    LikedSongsLoaded {
        session: AuthorizedSession,
        page: Page<TrackSummary>,
        append: bool,
    },
    LikedSongsFailed(String),
    AlbumsLoaded {
        session: AuthorizedSession,
        page: Page<PlaylistSummary>,
        append: bool,
    },
    AlbumsFailed(String),
    FollowingLoaded {
        session: AuthorizedSession,
        page: Page<UserSummary>,
        append: bool,
    },
    FollowingFailed(String),
    PlaylistsLoaded {
        session: AuthorizedSession,
        page: Page<PlaylistSummary>,
        append: bool,
    },
    PlaylistsFailed(String),
    PlaylistTracksLoaded {
        session: AuthorizedSession,
        playlist_urn: String,
        page: Page<TrackSummary>,
        append: bool,
    },
    PlaylistTracksFailed {
        playlist_urn: String,
        error: String,
    },
    UserTracksLoaded {
        session: AuthorizedSession,
        user_urn: String,
        page: Page<TrackSummary>,
        append: bool,
    },
    UserTracksFailed {
        user_urn: String,
        error: String,
    },
    UserPlaylistsLoaded {
        session: AuthorizedSession,
        user_urn: String,
        page: Page<PlaylistSummary>,
        append: bool,
    },
    UserPlaylistsFailed {
        user_urn: String,
        error: String,
    },
    SearchLoaded {
        session: AuthorizedSession,
        query: String,
        results: SearchResults,
    },
    SearchFailed {
        query: String,
        error: String,
    },
    SearchTracksPageLoaded {
        session: AuthorizedSession,
        query: String,
        page: Page<TrackSummary>,
    },
    SearchTracksPageFailed {
        query: String,
        error: String,
    },
    TrackLiked {
        session: AuthorizedSession,
        track_title: String,
    },
    TrackLikeFailed {
        track_title: String,
        error: String,
    },
    TrackAddedToPlaylist {
        session: AuthorizedSession,
        playlist_urn: String,
        playlist_title: String,
        track_title: String,
        already_present: bool,
    },
    TrackAddToPlaylistFailed {
        playlist_title: String,
        track_title: String,
        error: String,
    },
    ClipboardCopied {
        label: String,
    },
    ClipboardCopyFailed {
        label: String,
        error: String,
    },
    CoverArtLoaded {
        url: String,
        bytes: Vec<u8>,
    },
    CoverArtFailed {
        url: String,
        error: String,
    },
    PlaybackQueued {
        session: AuthorizedSession,
        title: String,
        preview: bool,
    },
    PlaybackFailed {
        title: String,
        error: String,
    },
    PlaybackIntent(PlaybackIntent),
    Player(PlayerEvent),
}
