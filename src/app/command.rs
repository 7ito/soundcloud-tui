use crate::{
    config::{
        credentials::Credentials, history::RecentlyPlayedStore, settings::Settings,
        tokens::TokenStore,
    },
    player::command::PlayerCommand,
    soundcloud::auth::{AuthorizationRequest, AuthorizedSession},
    soundcloud::models::{PlaylistSummary, TrackSummary},
};

#[derive(Debug, Clone)]
pub enum AppCommand {
    OpenUrl(String),
    SaveCredentials(AuthorizationRequest),
    ValidateSavedSession {
        credentials: Credentials,
        tokens: TokenStore,
    },
    WaitForOAuthCallback(AuthorizationRequest),
    ExchangeAuthorizationCode {
        request: AuthorizationRequest,
        callback_input: String,
    },
    SaveSettings(Settings),
    SaveHistory(RecentlyPlayedStore),
    LoadFeed {
        session: AuthorizedSession,
        next_href: Option<String>,
        append: bool,
    },
    LoadLikedSongs {
        session: AuthorizedSession,
        next_href: Option<String>,
        append: bool,
    },
    LoadAlbums {
        session: AuthorizedSession,
        next_href: Option<String>,
        append: bool,
    },
    LoadFollowing {
        session: AuthorizedSession,
        next_href: Option<String>,
        append: bool,
    },
    LoadPlaylists {
        session: AuthorizedSession,
        next_href: Option<String>,
        append: bool,
    },
    LoadPlaylistTracks {
        session: AuthorizedSession,
        playlist_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    LoadUserTracks {
        session: AuthorizedSession,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    LoadUserPlaylists {
        session: AuthorizedSession,
        user_urn: String,
        next_href: Option<String>,
        append: bool,
    },
    SearchAll {
        session: AuthorizedSession,
        query: String,
    },
    SearchTracksPage {
        session: AuthorizedSession,
        query: String,
        next_href: String,
    },
    LikeTrack {
        session: AuthorizedSession,
        track: TrackSummary,
    },
    AddTrackToPlaylist {
        session: AuthorizedSession,
        track: TrackSummary,
        playlist: PlaylistSummary,
    },
    CopyText {
        text: String,
        label: String,
    },
    LoadCoverArt {
        url: String,
    },
    PlayTrack {
        session: AuthorizedSession,
        track: TrackSummary,
    },
    ControlPlayback(PlayerCommand),
}
