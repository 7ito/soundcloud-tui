impl AppState {
    pub fn new() -> Self {
        Self::new_with_persistence(Settings::default(), RecentlyPlayedStore::default())
    }

    pub fn new_with_persistence(
        mut settings: Settings,
        recent_history: RecentlyPlayedStore,
    ) -> Self {
        settings.normalize();
        let library_items = vec![
            LibraryItem {
                label: "Feed",
                route: Route::Feed,
            },
            LibraryItem {
                label: "Liked Songs",
                route: Route::LikedSongs,
            },
            LibraryItem {
                label: "Recently Played",
                route: Route::RecentlyPlayed,
            },
            LibraryItem {
                label: "Albums",
                route: Route::Albums,
            },
            LibraryItem {
                label: "Following",
                route: Route::Following,
            },
        ];

        let playlists = mock_playlists();
        let feed_rows = mock_track_rows(&[
            ("Canvas", "Tourist", "From Nils Frahm repost", "4:11"),
            ("Pillar", "Bonobo", "Friends upload", "3:54"),
            ("Tied", "Ross From Friends", "Weekend rotation", "5:26"),
            ("Float", "TSHA", "Daily feed", "4:07"),
            ("Luna Park", "Logic1000", "Club notes", "3:43"),
        ]);
        let liked_rows = mock_track_rows(&[
            ("Whisper", "Fred again..", "Liked Songs", "3:39"),
            ("Mosaic", "Overmono", "Liked Songs", "4:23"),
            ("Blue Drift", "Dj Seinfeld", "Liked Songs", "5:01"),
            ("Signal", "Four Tet", "Liked Songs", "4:34"),
            ("Tides", "Caribou", "Liked Songs", "4:56"),
        ]);
        let recent_rows = mock_track_rows(&[
            ("Heatmap", "Bicep", "Yesterday afternoon", "4:20"),
            ("Harbor", "Tycho", "Morning focus", "5:05"),
            ("Bodyline", "Jamie xx", "Late-night coding", "3:58"),
            ("Avenue", "Kelly Lee Owens", "Commute", "4:14"),
            ("Driftglass", "George Fitzgerald", "Coffee break", "4:33"),
        ]);
        let album_rows = mock_track_rows(&[
            ("Fragments", "Bonobo", "12 tracks", "2022"),
            ("Singularity", "Jon Hopkins", "10 tracks", "2018"),
            ("Immunity", "Jon Hopkins", "8 tracks", "2013"),
            ("Awake", "Tycho", "8 tracks", "2014"),
        ]);
        let following_rows = mock_track_rows(&[
            ("Ross From Friends", "184K", "New EP teaser", "Online"),
            ("TSHA", "221K", "Tour diary", "Online"),
            ("Tourist", "129K", "Studio notes", "Away"),
            ("Ford.", "96K", "Ambient session", "Online"),
        ]);
        let search_rows = mock_track_rows(&[
            ("Sketch One", "Model Man", "Search result", "3:17"),
            ("Sketch Two", "Daniel Avery", "Search result", "4:42"),
            ("Sketch Three", "Kettama", "Search result", "3:56"),
            ("Sketch Four", "Brijean", "Search result", "4:18"),
            ("Sketch Five", "Maribou State", "Search result", "5:09"),
        ]);

        Self {
            mode: AppMode::Main,
            route: Route::Feed,
            focus: Focus::Library,
            should_quit: false,
            show_help: false,
            settings_menu: None,
            show_welcome: true,
            error_modal: None,
            add_to_playlist_modal: None,
            logout_confirm_modal: None,
            toast: None,
            help_scroll: 0,
            auth: AuthState::new(Credentials::default()),
            session: None,
            auth_summary: "Unauthenticated".to_string(),
            status: "Tab cycles panes, / opens search, v opens visualizer, z queues selected, Q opens queue, w/l use selected track, W/L use now playing, ? opens help, q quits."
                .to_string(),
            tick_count: 0,
            viewport: Viewport {
                width: 0,
                height: 0,
            },
            last_mouse_click: None,
            loading: Some(LoadingState {
                message: "Preparing mock library data...".to_string(),
                ticks_remaining: 2,
            }),
            search_query: String::new(),
            search_cursor: 0,
            search_return_focus: Focus::Library,
            library_items,
            playlists,
            feed_rows,
            liked_rows,
            recent_rows,
            album_rows,
            following_rows,
            search_rows,
            selected_library: 0,
            selected_playlist: 0,
            selected_content: 0,
            layout: LayoutState::default(),
            now_playing: NowPlaying {
                track: None,
                title: "Nothing playing".to_string(),
                artist: "Select a track and press Enter".to_string(),
                context: "Idle".to_string(),
                artwork_url: None,
                elapsed_label: "0:00".to_string(),
                duration_label: "0:00".to_string(),
                progress_ratio: 0.0,
            },
            cover_art: CoverArt::default(),
            player: PlayerState {
                status: PlaybackStatus::Stopped,
                volume_percent: 50.0,
                position_seconds: 0.0,
                duration_seconds: None,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
            },
            queue: QueueState::default(),
            visualizer: VisualizerState::default(),
            settings,
            help_requires_acknowledgement: false,
            content_return_focus: Focus::Library,
            pending_commands: VecDeque::new(),
            recent_history,
            active_playlist_urn: None,
            known_playlists: HashMap::new(),
            feed: CollectionState::default(),
            liked_tracks: CollectionState::default(),
            albums: CollectionState::default(),
            following: CollectionState::default(),
            playlist_tracks: HashMap::new(),
            search_tracks: CollectionState::default(),
            search_playlists: CollectionState::default(),
            search_users: CollectionState::default(),
            search_view: SearchView::Tracks,
            active_user_profile: None,
            user_profile_tracks: CollectionState::default(),
            user_profile_playlists: CollectionState::default(),
            user_profile_view: UserProfileView::Tracks,
            search_cache: HashMap::new(),
            playback_plan: PlaybackPlanState::default(),
            playlists_loading: false,
            playlists_loaded: false,
            playlists_error: None,
            playlists_next_href: None,
        }
    }

    pub fn new_onboarding(credentials: Credentials) -> Self {
        Self::new_onboarding_with_persistence(
            credentials,
            Settings::default(),
            RecentlyPlayedStore::default(),
        )
    }

    pub fn new_onboarding_with_persistence(
        credentials: Credentials,
        settings: Settings,
        recent_history: RecentlyPlayedStore,
    ) -> Self {
        let mut app = Self::new_with_persistence(settings, recent_history);
        app.mode = AppMode::Auth;
        app.auth = AuthState::new(credentials);
        app.auth_summary = "Not authenticated yet".to_string();
        app.loading = None;
        app.status = "Finish SoundCloud setup to enter the main player shell.".to_string();
        app
    }

    pub fn apply(&mut self, action: Action) {
        reducer::reduce(self, action);
    }
}
