impl AppState {
    pub fn current_content(&self) -> ContentView {
        if self.session.is_none() {
            return self.mock_content();
        }

        match self.route {
            Route::Feed => ContentView {
                title: "Feed".to_string(),
                subtitle: "Recent track activity from the accounts you follow.".to_string(),
                columns: ["Title", "Artist", "Source", "Length"],
                rows: self
                    .feed
                    .items
                    .iter()
                    .map(|item| ContentRow {
                        columns: match &item.origin {
                            FeedOrigin::Track(track) => [
                                track.title.clone(),
                                track.artist.clone(),
                                pretty_activity_type(&item.activity_type),
                                track.duration_label(),
                            ],
                            FeedOrigin::Playlist(playlist) => [
                                playlist.title.clone(),
                                playlist.creator.clone(),
                                pretty_activity_type(&item.activity_type),
                                playlist.track_count_label(),
                            ],
                        },
                    })
                    .collect(),
                state_label: self.feed.state_label(),
                empty_message: "No feed activity is available right now.".to_string(),
                help_message: Some(
                    "Enter plays tracks and opens playlists when the feed row points to one."
                        .to_string(),
                ),
            },
            Route::LikedSongs => ContentView {
                title: "Liked Songs".to_string(),
                subtitle: "Tracks you have liked on SoundCloud.".to_string(),
                columns: ["Title", "Artist", "Access", "Length"],
                rows: self
                    .liked_tracks
                    .items
                    .iter()
                    .map(track_row_with_access)
                    .collect(),
                state_label: self.liked_tracks.state_label(),
                empty_message: "No liked tracks were returned by SoundCloud.".to_string(),
                help_message: Some("Press Enter to play from your liked queue.".to_string()),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local playback history stored on this machine.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Played"],
                rows: self
                    .recent_history
                    .entries
                    .iter()
                    .map(history_row)
                    .collect(),
                state_label: self.recent_history_state_label(),
                empty_message: "Play a track to start building local history.".to_string(),
                help_message: Some(
                    "Recently Played persists across restarts and seeds a playable queue."
                        .to_string(),
                ),
            },
            Route::Albums => ContentView {
                title: "Albums".to_string(),
                subtitle: "Album-like playlists derived from your SoundCloud library.".to_string(),
                columns: ["Album", "Creator", "Tracks", "Year"],
                rows: self
                    .albums
                    .items
                    .iter()
                    .map(|playlist| ContentRow {
                        columns: [
                            playlist.title.clone(),
                            playlist.creator.clone(),
                            playlist.track_count_label(),
                            playlist.year_label(),
                        ],
                    })
                    .collect(),
                state_label: self.albums.state_label(),
                empty_message: "No album-like playlists were found.".to_string(),
                help_message: Some("Press Enter to open an album as playlist detail.".to_string()),
            },
            Route::Following => ContentView {
                title: "Following".to_string(),
                subtitle: "Accounts you follow on SoundCloud.".to_string(),
                columns: ["Creator", "Followers", "Catalog", "Profile"],
                rows: self
                    .following
                    .items
                    .iter()
                    .map(|user| ContentRow {
                        columns: [
                            user.username.clone(),
                            user.followers_label(),
                            user.spotlight_label(),
                            if user.permalink_url.is_some() {
                                "Profile".to_string()
                            } else {
                                "--".to_string()
                            },
                        ],
                    })
                    .collect(),
                state_label: self.following.state_label(),
                empty_message: "This account is not following anyone yet.".to_string(),
                help_message: Some(
                    "Press Enter to open a followed creator profile in the TUI.".to_string(),
                ),
            },
            Route::Playlist => {
                let playlist = self.active_playlist();
                let tracks = playlist
                    .map(|playlist| playlist.urn.as_str())
                    .and_then(|urn| self.playlist_tracks.get(urn));

                ContentView {
                    title: playlist
                        .map(|playlist| playlist.title.clone())
                        .unwrap_or_else(|| "Playlist".to_string()),
                    subtitle: playlist
                        .map(playlist_summary_subtitle)
                        .unwrap_or_else(|| "Playlist details are loading.".to_string()),
                    columns: ["Title", "Artist", "Access", "Length"],
                    rows: tracks
                        .map(|state| state.items.iter().map(track_row_with_access).collect())
                        .unwrap_or_default(),
                    state_label: tracks
                        .map(CollectionState::state_label)
                        .unwrap_or_else(|| "Waiting".to_string()),
                    empty_message: "No tracks are available for this playlist.".to_string(),
                    help_message: Some(
                        "Enter plays from this playlist queue. Esc returns to the prior pane."
                            .to_string(),
                    ),
                }
            }
            Route::UserProfile => {
                let Some(user) = self.active_user_profile.as_ref() else {
                    return ContentView {
                        title: "Profile".to_string(),
                        subtitle: "Select a creator from Following or Search to open a profile."
                            .to_string(),
                        columns: ["Title", "Artist", "Access", "Length"],
                        rows: Vec::new(),
                        state_label: "Waiting".to_string(),
                        empty_message: "No creator profile is currently open.".to_string(),
                        help_message: Some(
                            "Press 1 for tracks and 2 for playlists once a profile is open."
                                .to_string(),
                        ),
                    };
                };

                match self.user_profile_view {
                    UserProfileView::Tracks => ContentView {
                        title: format!("{} - Tracks", user.username),
                        subtitle: self.user_profile_subtitle(user),
                        columns: ["Title", "Artist", "Access", "Length"],
                        rows: self
                            .user_profile_tracks
                            .items
                            .iter()
                            .map(track_row_with_access)
                            .collect(),
                        state_label: self.user_profile_tracks.state_label(),
                        empty_message: format!(
                            "No public tracks are available for {}.",
                            user.username
                        ),
                        help_message: Some(
                            "1 tracks | 2 playlists | Enter plays the selected track.".to_string(),
                        ),
                    },
                    UserProfileView::Playlists => ContentView {
                        title: format!("{} - Playlists", user.username),
                        subtitle: self.user_profile_subtitle(user),
                        columns: ["Playlist", "Creator", "Tracks", "Year"],
                        rows: self
                            .user_profile_playlists
                            .items
                            .iter()
                            .map(playlist_row)
                            .collect(),
                        state_label: self.user_profile_playlists.state_label(),
                        empty_message: format!(
                            "No public playlists are available for {}.",
                            user.username
                        ),
                        help_message: Some(
                            "1 tracks | 2 playlists | Enter opens the selected playlist."
                                .to_string(),
                        ),
                    },
                }
            }
            Route::Search => match self.search_view {
                SearchView::Tracks => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Title", "Artist", "Access", "Length"],
                    rows: self
                        .search_tracks
                        .items
                        .iter()
                        .map(track_row_with_access)
                        .collect(),
                    state_label: self.search_tracks.state_label(),
                    empty_message: "No matching tracks were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
                SearchView::Playlists => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Playlist", "Creator", "Tracks", "Year"],
                    rows: self
                        .search_playlists
                        .items
                        .iter()
                        .map(playlist_row)
                        .collect(),
                    state_label: self.search_playlists.state_label_with_more_available(false),
                    empty_message: "No matching playlists were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
                SearchView::Users => ContentView {
                    title: self.search_title(),
                    subtitle: self.search_subtitle(),
                    columns: ["Creator", "Followers", "Catalog", "Profile"],
                    rows: self.search_users.items.iter().map(user_row).collect(),
                    state_label: self.search_users.state_label_with_more_available(false),
                    empty_message: "No matching creators were found.".to_string(),
                    help_message: Some(self.search_help_message()),
                },
            },
        }
    }

    pub fn current_content_len(&self) -> usize {
        self.current_content().rows.len()
    }

    pub fn current_content_row(&self) -> Option<ContentRow> {
        self.current_content()
            .rows
            .get(self.selected_content)
            .cloned()
    }

    pub fn current_selection_label(&self) -> Option<String> {
        match self.current_selected_content() {
            Some(SelectedContent::Track { track, .. }) => Some(track.title),
            Some(SelectedContent::Playlist(playlist)) => Some(playlist.title),
            Some(SelectedContent::User(user)) => Some(user.username),
            None => self.current_content_row().map(|row| row.columns[0].clone()),
        }
    }

    pub fn header_help_label(&self) -> String {
        if self.show_help {
            format!(
                "Esc closes help | {}/{} scroll | {} toggles",
                self.settings.keybinding(KeyAction::NextPage),
                self.settings.keybinding(KeyAction::PreviousPage),
                self.settings.keybinding(KeyAction::Help)
            )
        } else {
            format!(
                "{} help | {} search | v visualizer | {} queue | {} overlay | w/l selected | W/L current | Tab panes | j/k move | Enter select | q quit",
                self.settings.keybinding(KeyAction::Help),
                self.settings.keybinding(KeyAction::Search),
                self.settings.keybinding(KeyAction::AddToQueue),
                self.settings.keybinding(KeyAction::ShowQueue)
            )
        }
    }

    pub fn help_rows(&self) -> Vec<HelpRow> {
        vec![
            help_row("Move focus to next pane", "Tab", "Navigation"),
            help_row("Move focus to previous pane", "Shift+Tab", "Navigation"),
            help_row("Move selection down", "j | Down", "Navigation"),
            help_row("Move selection up", "k | Up", "Navigation"),
            help_row("Enter active pane", "Enter", "Navigation"),
            help_row(
                "Open help menu",
                format!("{} | F1", self.settings.keybinding(KeyAction::Help)),
                "General",
            ),
            help_row(
                "Enter input for search",
                self.settings.keybinding(KeyAction::Search),
                "General",
            ),
            help_row(
                "Pause/Resume playback",
                self.settings.keybinding(KeyAction::TogglePlayback),
                "General",
            ),
            help_row(
                "Open queue overlay",
                self.settings.keybinding(KeyAction::ShowQueue),
                "General",
            ),
            help_row("Toggle fullscreen visualizer", "v", "General"),
            help_row("Cycle visualizer style", "V", "Visualizer"),
            help_row("Add selected track to playlist", "w", "Content"),
            help_row("Add selected track to Liked Songs", "l", "Content"),
            help_row(
                "Add selected track to queue",
                self.settings.keybinding(KeyAction::AddToQueue),
                "Content",
            ),
            help_row("Add current now playing track to playlist", "W", "General"),
            help_row(
                "Add current now playing track to Liked Songs",
                "L",
                "General",
            ),
            help_row(
                "Skip to next track",
                self.settings.keybinding(KeyAction::NextTrack),
                "General",
            ),
            help_row(
                "Skip to previous track",
                self.settings.keybinding(KeyAction::PreviousTrack),
                "General",
            ),
            help_row(
                "Seek backwards",
                self.settings.keybinding(KeyAction::SeekBackwards),
                "General",
            ),
            help_row(
                "Seek forwards",
                self.settings.keybinding(KeyAction::SeekForwards),
                "General",
            ),
            help_row(
                "Increase volume",
                self.settings.keybinding(KeyAction::IncreaseVolume),
                "General",
            ),
            help_row(
                "Decrease volume",
                self.settings.keybinding(KeyAction::DecreaseVolume),
                "General",
            ),
            help_row(
                "Cycle repeat mode",
                self.settings.keybinding(KeyAction::Repeat),
                "General",
            ),
            help_row(
                "Toggle shuffle mode",
                self.settings.keybinding(KeyAction::Shuffle),
                "General",
            ),
            help_row(
                "Copy URL to currently playing track",
                self.settings.keybinding(KeyAction::CopySongUrl),
                "General",
            ),
            help_row(
                "Scroll down to next result page",
                self.settings.keybinding(KeyAction::NextPage),
                "Pagination",
            ),
            help_row(
                "Scroll up to previous result page",
                self.settings.keybinding(KeyAction::PreviousPage),
                "Pagination",
            ),
            help_row("Decrease sidebar width", "{", "Layout"),
            help_row("Increase sidebar width", "}", "Layout"),
            help_row("Decrease playbar or library height", "(", "Layout"),
            help_row("Increase playbar or library height", ")", "Layout"),
            help_row("Reset layout to defaults", "|", "Layout"),
            help_row(
                "Open settings menu",
                self.settings.keybinding(KeyAction::OpenSettings),
                "General",
            ),
            help_row("Search with input text", "Enter", "Search input"),
            help_row("Delete entire input", "Ctrl+l", "Search input"),
            help_row(
                "Delete text from cursor to start of input",
                "Ctrl+u",
                "Search input",
            ),
            help_row(
                "Delete text from cursor to end of input",
                "Ctrl+k",
                "Search input",
            ),
            help_row("Delete previous word", "Ctrl+w", "Search input"),
            help_row("Jump to start of input", "Ctrl+i", "Search input"),
            help_row("Jump to end of input", "Ctrl+o", "Search input"),
            help_row(
                "Escape from input back to hovered block",
                "Esc",
                "Search input",
            ),
            help_row(
                "Escape back to previously navigated pane",
                "Esc",
                "Tracks/Album/User",
            ),
            help_row("Play the selected queued track", "Enter", "Queue overlay"),
            help_row("Remove the selected queued track", "d", "Queue overlay"),
        ]
    }

    pub fn help_row_count(&self) -> usize {
        self.help_rows().len()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn theme(&self) -> Theme {
        Theme::from_settings(&self.settings)
    }

    pub fn show_settings(&self) -> bool {
        self.settings_menu.is_some()
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn focus_content_from(&mut self, focus: Focus) {
        self.content_return_focus = focus;
        self.focus = Focus::Content;
    }

    pub fn playlist_panel_title(&self) -> String {
        if self.playlists_loading {
            "Playlists (loading...)".to_string()
        } else if self.playlists_error.is_some() {
            "Playlists (error)".to_string()
        } else if self.playlists_loaded && self.playlists.is_empty() {
            "Playlists (empty)".to_string()
        } else if self.playlists_next_href.is_some() {
            format!("Playlists ({}, more available)", self.playlists.len())
        } else {
            format!("Playlists ({})", self.playlists.len())
        }
    }

    pub fn playlist_panel_placeholder(&self) -> Option<String> {
        if self.playlists_loading && self.playlists.is_empty() {
            Some("Loading playlists...".to_string())
        } else if self.playlists_error.is_some() {
            Some("Could not load playlists. Press F5 to retry.".to_string())
        } else if self.playlists_loaded && self.playlists.is_empty() {
            Some("No playlists are available for this account yet.".to_string())
        } else {
            None
        }
    }

    pub fn is_sidebar_playlist_active(&self, index: usize) -> bool {
        self.route == Route::Playlist
            && self
                .playlists
                .get(index)
                .and_then(|playlist| playlist.urn.as_deref())
                == self.active_playlist_urn.as_deref()
    }

    pub fn queue_status_label(&self) -> String {
        let queue_len = self.visible_queue_indices().len();
        let queue_position = if matches!(
            self.current_playback_plan_item().map(|item| item.kind),
            Some(PlaybackPlanItemKind::Queue)
        ) {
            format!("1/{}", queue_len.max(1))
        } else {
            format!("0/{}", queue_len)
        };

        format!(
            "Queue {} | Repeat {} | Shuffle {}",
            queue_position,
            self.player.repeat_mode.label(),
            if self.player.shuffle_enabled {
                self.settings.shuffle_icon.as_str()
            } else {
                "Off"
            }
        )
    }

    pub fn queue_overlay_rows(&self) -> Vec<ContentRow> {
        self.visible_queue_indices()
            .into_iter()
            .filter_map(|index| {
                self.playback_plan.items.get(index).map(|item| ContentRow {
                    columns: [
                        item.track.title.clone(),
                        item.track.artist.clone(),
                        self.queue_state_label_for_index(index),
                        item.track.duration_label(),
                    ],
                })
            })
            .collect()
    }

    pub fn queue_overlay_selection(&self) -> Option<usize> {
        (!self.visible_queue_indices().is_empty()).then_some(
            self.queue
                .selected
                .min(self.visible_queue_indices().len().saturating_sub(1)),
        )
    }

    pub fn can_play_next_track(&self) -> bool {
        self.next_playback_index().is_some()
    }

    pub fn can_play_previous_track(&self) -> bool {
        self.player.position_seconds > 5.0 || self.previous_playback_index().is_some()
    }

    fn current_playback_plan_item(&self) -> Option<&PlaybackPlanItem> {
        self.playback_plan
            .current_index
            .and_then(|index| self.playback_plan.items.get(index))
    }

    fn visible_queue_indices(&self) -> Vec<usize> {
        let start = match self.playback_plan.current_index {
            Some(index)
                if matches!(
                    self.playback_plan.items.get(index).map(|item| item.kind),
                    Some(PlaybackPlanItemKind::Queue)
                ) =>
            {
                index
            }
            Some(index) => index.saturating_add(1),
            None => 0,
        };

        self.playback_plan
            .items
            .iter()
            .enumerate()
            .skip(start)
            .filter_map(|(index, item)| (item.kind == PlaybackPlanItemKind::Queue).then_some(index))
            .collect()
    }

    fn pending_queue_items(&self) -> Vec<PlaybackPlanItem> {
        let start = self
            .playback_plan
            .current_index
            .map(|index| index.saturating_add(1))
            .unwrap_or(0);

        self.playback_plan
            .items
            .iter()
            .skip(start)
            .filter(|item| item.kind == PlaybackPlanItemKind::Queue)
            .cloned()
            .collect()
    }

    fn queue_state_label_for_index(&self, index: usize) -> String {
        if self.playback_plan.current_index == Some(index) {
            "Playing".to_string()
        } else if self
            .visible_queue_indices()
            .first()
            .copied()
            .is_some_and(|first| first == index)
        {
            "Next".to_string()
        } else {
            "Queued".to_string()
        }
    }

    fn build_plan_item(
        &self,
        track: TrackSummary,
        context: String,
        kind: PlaybackPlanItemKind,
    ) -> PlaybackPlanItem {
        PlaybackPlanItem {
            track,
            context,
            kind,
        }
    }

    fn rebuild_playback_plan_for_source(
        &mut self,
        tracks: Vec<TrackSummary>,
        current_index: usize,
        context: String,
    ) {
        let pending_queue = self.pending_queue_items();
        let mut items = tracks
            .into_iter()
            .map(|track| self.build_plan_item(track, context.clone(), PlaybackPlanItemKind::Source))
            .collect::<Vec<_>>();

        items.splice(
            current_index.saturating_add(1)..current_index.saturating_add(1),
            pending_queue,
        );
        self.playback_plan.items = items;
        self.playback_plan.current_index = Some(current_index);
        self.queue.selected = 0;
    }

    fn append_track_to_queue(&mut self, track: TrackSummary) {
        let item = self.build_plan_item(
            track.clone(),
            "Queue".to_string(),
            PlaybackPlanItemKind::Queue,
        );

        if let Some(current_index) = self.playback_plan.current_index {
            let mut insert_at = current_index.saturating_add(1);
            while self
                .playback_plan
                .items
                .get(insert_at)
                .is_some_and(|existing| existing.kind == PlaybackPlanItemKind::Queue)
            {
                insert_at += 1;
            }
            self.playback_plan.items.insert(insert_at, item);
        } else {
            self.playback_plan.items.push(item);
        }

        self.queue.selected = self.visible_queue_indices().len().saturating_sub(1);
        self.status = format!("Queued {}.", track.title);
    }

    pub fn help_visible_rows(&self) -> usize {
        self.viewport.height.saturating_sub(7).max(8) as usize
    }

    pub fn sync_route_from_library(&mut self) {
        if let Some(item) = self.library_items.get(self.selected_library) {
            self.set_route(item.route);
        }
    }

    pub fn sync_route_from_playlist(&mut self) {
        if self.playlists.is_empty() {
            return;
        }

        let playlist_index = self
            .selected_playlist
            .min(self.playlists.len().saturating_sub(1));
        self.selected_playlist = playlist_index;

        if let Some(playlist) = self.playlists.get(playlist_index) {
            self.active_playlist_urn = playlist.urn.clone();
        }

        self.set_route(Route::Playlist);
    }

    pub fn set_route(&mut self, route: Route) {
        self.route = route;
        self.selected_content = 0;
        self.status = format!("Browsing {}.", self.route_title());

        if self.session.is_some() {
            self.request_route_load(false);
        } else {
            self.set_loading(format!("Loading mock data for {}...", self.route_title()));
        }
    }

    pub fn route_title(&self) -> String {
        match self.route {
            Route::Playlist => self
                .active_playlist()
                .map(|playlist| playlist.title.clone())
                .or_else(|| {
                    self.playlists
                        .get(self.selected_playlist)
                        .map(|playlist| playlist.title.clone())
                })
                .unwrap_or_else(|| "Playlist".to_string()),
            Route::UserProfile => self
                .active_user_profile
                .as_ref()
                .map(|user| user.username.clone())
                .unwrap_or_else(|| "Profile".to_string()),
            _ => self.route.label().to_string(),
        }
    }

    pub fn select_current_content(&mut self) {
        match self.current_selected_content() {
            Some(SelectedContent::Track { track, context }) => {
                if let Some((tracks, current_index)) = self.current_track_queue_selection() {
                    self.rebuild_playback_plan_for_source(tracks, current_index, context.clone());
                } else {
                    self.rebuild_playback_plan_for_source(vec![track.clone()], 0, context.clone());
                }
                self.start_track_playback(track, context);
            }
            Some(SelectedContent::Playlist(playlist)) => {
                self.open_playlist(playlist);
            }
            Some(SelectedContent::User(user)) => {
                self.open_user_profile(user);
            }
            None => {
                let Some(row) = self.current_content_row() else {
                    self.status = "Nothing selected in the content pane.".to_string();
                    return;
                };

                if self.route.is_track_view() {
                    self.status = format!(
                        "Playback is unavailable until SoundCloud authentication is complete for {}.",
                        row.columns[0]
                    );
                } else {
                    self.status = format!("Inspected {}.", row.columns[0]);
                }
            }
        }
    }
    fn remember_playlist(&mut self, playlist: SoundcloudPlaylist) {
        self.known_playlists.insert(playlist.urn.clone(), playlist);
    }

    fn active_playlist(&self) -> Option<&SoundcloudPlaylist> {
        self.active_playlist_urn
            .as_ref()
            .and_then(|urn| self.known_playlists.get(urn))
    }

    fn open_playlist(&mut self, playlist: SoundcloudPlaylist) {
        let urn = playlist.urn.clone();
        self.remember_playlist(playlist.clone());
        self.active_playlist_urn = Some(urn.clone());

        if let Some(index) = self
            .playlists
            .iter()
            .position(|sidebar| sidebar.urn.as_deref() == Some(urn.as_str()))
        {
            self.selected_playlist = index;
        }

        self.set_route(Route::Playlist);
        self.status = format!("Opened playlist {}.", playlist.title);
    }

    fn open_user_profile(&mut self, user: UserSummary) {
        self.active_user_profile = Some(user.clone());
        self.user_profile_tracks = CollectionState::default();
        self.user_profile_playlists = CollectionState::default();
        self.user_profile_view = UserProfileView::Tracks;
        self.set_route(Route::UserProfile);
        self.status = format!("Opened {}'s profile.", user.username);
    }

    fn active_user_profile_urn(&self) -> Option<&str> {
        self.active_user_profile
            .as_ref()
            .map(|user| user.urn.as_str())
    }

    fn recent_history_state_label(&self) -> String {
        if self.recent_history.entries.is_empty() {
            "Empty".to_string()
        } else {
            format!("Loaded {} local plays", self.recent_history.entries.len())
        }
    }

    fn search_subtitle(&self) -> String {
        format!(
            "Showing {} for '{}'. Tracks: {} | Playlists: {} | Users: {}",
            self.search_view.label(),
            self.search_query,
            self.search_tracks.items.len(),
            self.search_playlists.items.len(),
            self.search_users.items.len(),
        )
    }

    fn search_title(&self) -> String {
        if self.search_query.is_empty() {
            "Search".to_string()
        } else {
            format!("Search: {}", self.search_query)
        }
    }

    fn search_help_message(&self) -> String {
        let pagination = if self.search_view == SearchView::Tracks {
            "Use Ctrl+d and Ctrl+u to jump across result pages."
        } else {
            "Playlist and creator results are first-page snapshots for now."
        };

        format!(
            "Press / to refine the query and 1/2/3 to switch search tables. {} Enter opens playlists or profiles.",
            pagination
        )
    }

    fn set_search_view(&mut self, search_view: SearchView) {
        self.search_view = search_view;
        self.selected_content = 0;
        self.status = format!("Showing {} search results.", search_view.label());
    }

    fn set_user_profile_view(&mut self, user_profile_view: UserProfileView) {
        if self.user_profile_view == user_profile_view {
            return;
        }

        self.user_profile_view = user_profile_view;
        self.selected_content = 0;
        self.status = format!(
            "Showing {} for {}.",
            self.user_profile_view.label(),
            self.route_title()
        );
        self.request_route_load(false);
    }

    fn user_profile_subtitle(&self, user: &UserSummary) -> String {
        let mut segments = vec![
            format!("Followers {}", user.followers_label()),
            format!("{} tracks", user.track_count),
            format!("{} playlists", user.playlist_count),
            format!("Showing {}", self.user_profile_view.label()),
        ];

        if let Some(permalink) = user.permalink_url.as_deref() {
            segments.push(permalink.to_string());
        }

        segments.join(" | ")
    }

    fn cache_search_results(&mut self) {
        if self.search_query.trim().is_empty() {
            return;
        }

        self.search_cache
            .insert(self.search_query.clone(), SearchCache::from_state(self));
    }
    fn current_selected_content(&self) -> Option<SelectedContent> {
        let index = self.selected_content;

        if self.session.is_none() {
            return None;
        }

        match self.route {
            Route::Feed => self
                .feed
                .items
                .get(index)
                .and_then(|item| match &item.origin {
                    FeedOrigin::Track(track) => Some(SelectedContent::Track {
                        track: track.clone(),
                        context: pretty_activity_type(&item.activity_type),
                    }),
                    FeedOrigin::Playlist(playlist) => {
                        Some(SelectedContent::Playlist(playlist.clone()))
                    }
                }),
            Route::LikedSongs => {
                self.liked_tracks
                    .items
                    .get(index)
                    .cloned()
                    .map(|track| SelectedContent::Track {
                        track,
                        context: "Liked Songs".to_string(),
                    })
            }
            Route::RecentlyPlayed => self
                .recent_history
                .entries
                .get(index)
                .cloned()
                .map(|entry| SelectedContent::Track {
                    track: entry.track,
                    context: entry.context,
                }),
            Route::Albums => self
                .albums
                .items
                .get(index)
                .cloned()
                .map(SelectedContent::Playlist),
            Route::Following => self
                .following
                .items
                .get(index)
                .cloned()
                .map(SelectedContent::User),
            Route::Playlist => self
                .active_playlist_urn
                .as_ref()
                .and_then(|urn| self.playlist_tracks.get(urn))
                .and_then(|state| state.items.get(index))
                .cloned()
                .map(|track| SelectedContent::Track {
                    context: self.route_title(),
                    track,
                }),
            Route::UserProfile => match self.user_profile_view {
                UserProfileView::Tracks => {
                    self.user_profile_tracks
                        .items
                        .get(index)
                        .cloned()
                        .map(|track| SelectedContent::Track {
                            context: self.route_title(),
                            track,
                        })
                }
                UserProfileView::Playlists => self
                    .user_profile_playlists
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::Playlist),
            },
            Route::Search => match self.search_view {
                SearchView::Tracks => self.search_tracks.items.get(index).cloned().map(|track| {
                    SelectedContent::Track {
                        track,
                        context: format!("Search: {}", self.search_query),
                    }
                }),
                SearchView::Playlists => self
                    .search_playlists
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::Playlist),
                SearchView::Users => self
                    .search_users
                    .items
                    .get(index)
                    .cloned()
                    .map(SelectedContent::User),
            },
        }
    }
    fn mock_content(&self) -> ContentView {
        match self.route {
            Route::Feed => ContentView {
                title: "Feed".to_string(),
                subtitle: "Fresh picks from the accounts you follow.".to_string(),
                columns: ["Title", "Artist", "Source", "Length"],
                rows: self.feed_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock feed rows available.".to_string(),
                help_message: Some("Mock feed preview.".to_string()),
            },
            Route::LikedSongs => ContentView {
                title: "Liked Songs".to_string(),
                subtitle: "Mock favorites pinned for the shell.".to_string(),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.liked_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock liked songs available.".to_string(),
                help_message: Some("Mock liked songs preview.".to_string()),
            },
            Route::RecentlyPlayed => ContentView {
                title: "Recently Played".to_string(),
                subtitle: "Local history will become real in a later phase.".to_string(),
                columns: ["Title", "Artist", "Last Context", "Length"],
                rows: self.recent_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock history available.".to_string(),
                help_message: Some("Mock recently played preview.".to_string()),
            },
            Route::Albums => ContentView {
                title: "Albums".to_string(),
                subtitle: "Album-like sets surfaced as a dedicated mock view.".to_string(),
                columns: ["Album", "Creator", "Tracks", "Year"],
                rows: self.album_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock albums available.".to_string(),
                help_message: Some("Mock albums preview.".to_string()),
            },
            Route::Following => ContentView {
                title: "Following".to_string(),
                subtitle: "Mock creator directory with a few followed accounts.".to_string(),
                columns: ["Creator", "Followers", "Spotlight", "Status"],
                rows: self.following_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock following rows available.".to_string(),
                help_message: Some("Mock following preview.".to_string()),
            },
            Route::Playlist => {
                let playlist = &self.playlists[self.selected_playlist];
                ContentView {
                    title: playlist.title.clone(),
                    subtitle: playlist.description.clone(),
                    columns: ["Title", "Artist", "Playlist", "Length"],
                    rows: playlist.tracks.clone(),
                    state_label: "Mock data".to_string(),
                    empty_message: "No mock playlist tracks available.".to_string(),
                    help_message: Some("Mock playlist browsing preview.".to_string()),
                }
            }
            Route::UserProfile => ContentView {
                title: "Profile".to_string(),
                subtitle: "Creator stats and releases will appear here after authentication."
                    .to_string(),
                columns: ["Title", "Artist", "Access", "Length"],
                rows: Vec::new(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock creator profile is available.".to_string(),
                help_message: Some("Press Enter on a creator to open their profile.".to_string()),
            },
            Route::Search => ContentView {
                title: self.search_title(),
                subtitle: format!(
                    "Showing {} for '{}'. Tracks: {} | Playlists: 0 | Users: 0",
                    self.search_view.label(),
                    self.search_query,
                    self.search_rows.len(),
                ),
                columns: ["Title", "Artist", "Collection", "Length"],
                rows: self.search_rows.clone(),
                state_label: "Mock data".to_string(),
                empty_message: "No mock search results available.".to_string(),
                help_message: Some("Press 1, 2, or 3 to switch search result tables.".to_string()),
            },
        }
    }
}
