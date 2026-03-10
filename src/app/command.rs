use crate::{
    config::{
        credentials::Credentials, history::RecentlyPlayedStore, settings::Settings,
        tokens::TokenStore,
    },
    player::command::PlayerCommand,
    soundcloud::auth::{AuthorizationRequest, AuthorizedSession},
    soundcloud::models::TrackSummary,
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
    SearchAll {
        session: AuthorizedSession,
        query: String,
    },
    SearchTracksPage {
        session: AuthorizedSession,
        query: String,
        next_href: String,
    },
    PlayTrack {
        session: AuthorizedSession,
        track: TrackSummary,
    },
    ControlPlayback(PlayerCommand),
}
