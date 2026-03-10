#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Focus {
    Search,
    Library,
    Playlists,
    Content,
    Playbar,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Search => Self::Library,
            Self::Library => Self::Playlists,
            Self::Playlists => Self::Content,
            Self::Content => Self::Playbar,
            Self::Playbar => Self::Search,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Search => Self::Playbar,
            Self::Library => Self::Search,
            Self::Playlists => Self::Library,
            Self::Content => Self::Playlists,
            Self::Playbar => Self::Content,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Playlists => "Playlists",
            Self::Content => "Content",
            Self::Playbar => "Now Playing",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum Route {
    #[default]
    Feed,
    LikedSongs,
    RecentlyPlayed,
    Albums,
    Following,
    Playlist,
    Search,
}

impl Route {
    pub fn label(self) -> &'static str {
        match self {
            Self::Feed => "Feed",
            Self::LikedSongs => "Liked Songs",
            Self::RecentlyPlayed => "Recently Played",
            Self::Albums => "Albums",
            Self::Following => "Following",
            Self::Playlist => "Playlist",
            Self::Search => "Search",
        }
    }

    pub fn is_track_view(self) -> bool {
        matches!(
            self,
            Self::Feed | Self::LikedSongs | Self::RecentlyPlayed | Self::Playlist | Self::Search
        )
    }
}
