impl AppState {
    fn start_track_playback(&mut self, track: TrackSummary, context: String) {
        if !track.can_attempt_playback() {
            self.status = format!(
                "{} is blocked on SoundCloud and cannot be streamed.",
                track.title
            );
            return;
        }

        let Some(session) = self.session.clone() else {
            return;
        };

        self.player.status = PlaybackStatus::Buffering;
        self.player.position_seconds = 0.0;
        self.player.duration_seconds = track.duration_ms.map(|duration| duration as f64 / 1000.0);
        self.now_playing = NowPlaying {
            track: Some(track.clone()),
            title: track.title.clone(),
            artist: track.artist.clone(),
            context,
            artwork_url: track.artwork_url.clone(),
            elapsed_label: "0:00".to_string(),
            duration_label: track.duration_label(),
            progress_ratio: 0.0,
        };
        self.refresh_cover_art(track.artwork_url.as_deref());
        self.status = format!("Resolving SoundCloud stream for {}...", track.title);
        self.queue_command(AppCommand::PlayTrack { session, track });
    }

    fn refresh_cover_art(&mut self, artwork_url: Option<&str>) {
        let Some(artwork_url) = artwork_url.map(str::trim).filter(|value| !value.is_empty()) else {
            self.cover_art = CoverArt::default();
            return;
        };

        if self.cover_art.url.as_deref() == Some(artwork_url) {
            return;
        }

        self.cover_art.url = Some(artwork_url.to_string());
        self.cover_art.bytes = None;
        self.cover_art.loading = true;
        self.queue_command(AppCommand::LoadCoverArt {
            url: artwork_url.to_string(),
        });
    }

    fn current_track_queue_selection(&self) -> Option<(Vec<TrackSummary>, usize)> {
        match self.route {
            Route::Feed => {
                let mut queue = Vec::new();
                let mut selected_queue_index = None;

                for (row_index, item) in self.feed.items.iter().enumerate() {
                    if let FeedOrigin::Track(track) = &item.origin {
                        if row_index == self.selected_content {
                            selected_queue_index = Some(queue.len());
                        }
                        queue.push(track.clone());
                    }
                }

                selected_queue_index.map(|selected| (queue, selected))
            }
            Route::LikedSongs => Some((self.liked_tracks.items.clone(), self.selected_content)),
            Route::RecentlyPlayed => Some((
                self.recent_history
                    .entries
                    .iter()
                    .map(|entry| entry.track.clone())
                    .collect(),
                self.selected_content,
            )),
            Route::Playlist => {
                let urn = self.active_playlist_urn.as_ref()?;
                let tracks = self.playlist_tracks.get(urn)?.items.clone();
                Some((tracks, self.selected_content))
            }
            Route::Search if self.search_view == SearchView::Tracks => {
                Some((self.search_tracks.items.clone(), self.selected_content))
            }
            Route::UserProfile if self.user_profile_view == UserProfileView::Tracks => Some((
                self.user_profile_tracks.items.clone(),
                self.selected_content,
            )),
            Route::Search => None,
            Route::Albums | Route::Following | Route::UserProfile => None,
        }
        .and_then(|(tracks, selected)| {
            if selected < tracks.len() {
                Some((tracks, selected))
            } else {
                None
            }
        })
    }

    pub fn toggle_playback(&mut self) {
        self.apply_playback_intent(PlaybackIntent::TogglePause);
    }

    pub fn apply_playback_intent(&mut self, intent: PlaybackIntent) {
        match intent {
            PlaybackIntent::Play => self.play_playback(),
            PlaybackIntent::Pause => self.pause_playback(),
            PlaybackIntent::TogglePause => {
                if self.player.status == PlaybackStatus::Stopped {
                    self.play_playback();
                } else if self.now_playing.track.is_none() {
                    self.status = "Select a track first.".to_string();
                } else {
                    self.queue_command(AppCommand::ControlPlayback(PlayerCommand::TogglePause));
                    self.status = "Toggling playback...".to_string();
                }
            }
            PlaybackIntent::Stop => self.stop_playback(),
            PlaybackIntent::Next => self.play_next_track(),
            PlaybackIntent::Previous => self.play_previous_track(),
            PlaybackIntent::SeekRelative { seconds } => self.seek_relative(seconds),
            PlaybackIntent::SeekAbsolute { seconds } => self.seek_absolute(seconds),
            PlaybackIntent::SetVolume { percent } => self.set_volume(percent),
            PlaybackIntent::SetShuffle(enabled) => self.set_shuffle(enabled),
            PlaybackIntent::SetRepeat(mode) => self.set_repeat_mode(mode),
        }
    }

    fn play_playback(&mut self) {
        let Some(track) = self.now_playing.track.clone() else {
            self.status = "Select a track first.".to_string();
            return;
        };

        match self.player.status {
            PlaybackStatus::Playing => {
                self.status = format!("{} is already playing.", track.title);
            }
            PlaybackStatus::Buffering => {
                self.status = format!("{} is still buffering.", track.title);
            }
            PlaybackStatus::Paused => {
                self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Play));
                self.status = format!("Resuming {}...", track.title);
            }
            PlaybackStatus::Stopped => {
                self.start_track_playback(track, self.now_playing.context.clone());
            }
        }
    }

    fn pause_playback(&mut self) {
        let Some(title) = self
            .now_playing
            .track
            .as_ref()
            .map(|track| track.title.clone())
        else {
            self.status = "Nothing is playing.".to_string();
            return;
        };

        if self.player.status == PlaybackStatus::Paused {
            self.status = format!("{title} is already paused.");
            return;
        }

        if self.player.status == PlaybackStatus::Stopped {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Pause));
        self.status = format!("Pausing {title}...");
    }

    fn stop_playback(&mut self) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::Stop));
        self.status = "Stopping playback...".to_string();
    }

    fn seek_relative(&mut self, seconds: f64) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekRelative {
            seconds,
        }));
        self.status = if seconds.is_sign_negative() {
            format!("Seeking backward {:.0} seconds...", seconds.abs())
        } else {
            format!("Seeking forward {:.0} seconds...", seconds)
        };
    }

    fn seek_absolute(&mut self, seconds: f64) {
        if self.now_playing.track.is_none() {
            self.status = "Nothing is playing.".to_string();
            return;
        }

        let seconds = seconds.max(0.0);
        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekAbsolute {
            seconds,
        }));
        self.status = format!("Seeking to {}...", format_seconds_f64(seconds));
    }

    fn set_volume(&mut self, percent: f64) {
        let target = percent.clamp(0.0, 100.0);
        self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SetVolume {
            percent: target,
        }));
        self.status = format!("Setting volume to {:.0}%...", target.round());
    }

    fn set_shuffle(&mut self, enabled: bool) {
        self.player.shuffle_enabled = enabled;
        self.status = if enabled {
            "Shuffle enabled.".to_string()
        } else {
            "Shuffle disabled.".to_string()
        };
    }

    fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        self.player.repeat_mode = repeat_mode;
        self.status = format!("Repeat mode set to {}.", repeat_mode.label());
    }

    fn play_next_track(&mut self) {
        let Some(next_index) = self.next_playback_index() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(next_index).cloned() else {
            self.status = "Reached the end of the queue.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(next_index);
        self.start_track_playback(item.track, item.context);
    }

    fn next_playback_index(&self) -> Option<usize> {
        let current_index = self.playback_plan.current_index?;
        let track_count = self.playback_plan.items.len();

        if current_index + 1 < track_count {
            Some(current_index + 1)
        } else if self.player.repeat_mode == RepeatMode::Queue && track_count > 0 {
            Some(0)
        } else {
            None
        }
    }

    fn previous_playback_index(&self) -> Option<usize> {
        let current_index = self.playback_plan.current_index?;
        let track_count = self.playback_plan.items.len();

        if current_index > 0 {
            Some(current_index - 1)
        } else if self.player.repeat_mode == RepeatMode::Queue && track_count > 0 {
            Some(track_count.saturating_sub(1))
        } else {
            None
        }
    }

    fn restart_current_track(&mut self) -> bool {
        let track = self
            .current_playback_plan_item()
            .cloned()
            .map(|item| (item.track, item.context))
            .or_else(|| {
                self.now_playing
                    .track
                    .clone()
                    .map(|track| (track, self.now_playing.context.clone()))
            });

        let Some((track, context)) = track else {
            self.status = "Nothing queued for playback.".to_string();
            return false;
        };

        self.start_track_playback(track, context);
        true
    }

    fn play_previous_track(&mut self) {
        if self.playback_plan.current_index.is_none() {
            self.status = "Nothing queued for playback.".to_string();
            return;
        }

        if self.player.position_seconds > 5.0 {
            self.queue_command(AppCommand::ControlPlayback(PlayerCommand::SeekAbsolute {
                seconds: 0.0,
            }));
            self.status = "Restarting the current track.".to_string();
            return;
        }

        let Some(previous_index) = self.previous_playback_index() else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(previous_index).cloned() else {
            self.status = "Already at the start of the queue.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(previous_index);
        self.start_track_playback(item.track, item.context);
    }

    fn force_previous_track(&mut self) {
        if self.playback_plan.current_index.is_none() {
            self.status = "Nothing queued for playback.".to_string();
            return;
        }

        if let Some(previous_index) = self.previous_playback_index() {
            if let Some(item) = self.playback_plan.items.get(previous_index).cloned() {
                self.playback_plan.current_index = Some(previous_index);
                self.start_track_playback(item.track, item.context);
                return;
            }
        }

        let _ = self.restart_current_track();
    }

    fn apply_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::PlaybackStarted => {
                self.record_recent_playback();
                self.player.status = PlaybackStatus::Playing;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Playing {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackResumed => {
                self.player.status = PlaybackStatus::Playing;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Playing {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackPaused => {
                self.player.status = PlaybackStatus::Paused;
                if let Some(track) = &self.now_playing.track {
                    self.status = format!("Paused {}.", track.title);
                }
                self.sync_window_title();
            }
            PlayerEvent::PlaybackStopped => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                self.status = "Playback stopped.".to_string();
                self.sync_window_title();
            }
            PlayerEvent::TrackEnded => {
                self.player.status = PlaybackStatus::Stopped;
                self.player.position_seconds = 0.0;
                self.now_playing.elapsed_label = "0:00".to_string();
                self.now_playing.progress_ratio = 0.0;
                if self.settings.stop_after_current_track {
                    self.status = "Stopped after the current track.".to_string();
                } else if self.player.repeat_mode == RepeatMode::Track {
                    if !self.restart_current_track() {
                        self.status = "Reached the end of the queue.".to_string();
                    }
                } else if self.next_playback_index().is_some() {
                    self.play_next_track();
                } else {
                    self.status = "Reached the end of the queue.".to_string();
                }
            }
            PlayerEvent::PositionChanged { seconds } => {
                self.player.position_seconds = seconds;
                self.sync_now_playing_progress();
            }
            PlayerEvent::DurationChanged { seconds } => {
                self.player.duration_seconds = seconds;
                self.sync_now_playing_progress();
            }
            PlayerEvent::VolumeChanged { percent } => {
                self.player.volume_percent = percent.clamp(0.0, 100.0);
            }
            PlayerEvent::BackendError(error) => {
                self.player.status = PlaybackStatus::Stopped;
                let title = if crate::player::mpv_locator::is_missing_error_message(&error) {
                    "mpv is not installed"
                } else {
                    "Playback backend error"
                };
                self.show_main_error(title, error);
            }
        }
    }

    fn sync_now_playing_progress(&mut self) {
        self.now_playing.elapsed_label = format_seconds_f64(self.player.position_seconds);

        if let Some(duration_seconds) = self.player.duration_seconds {
            self.now_playing.duration_label = format_seconds_f64(duration_seconds);
            self.now_playing.progress_ratio = if duration_seconds <= 0.0 {
                0.0
            } else {
                (self.player.position_seconds / duration_seconds).clamp(0.0, 1.0)
            };
        }
    }
    fn selected_track_shortcut_target(&self) -> Option<TrackSummary> {
        if self.focus != Focus::Content {
            return None;
        }

        match self.current_selected_content() {
            Some(SelectedContent::Track { track, .. }) => Some(track),
            _ => None,
        }
    }

    fn now_playing_shortcut_target(&self) -> Option<TrackSummary> {
        self.now_playing.track.clone()
    }

    fn open_queue_overlay(&mut self) {
        self.queue.overlay_visible = true;
        self.queue.selected = self
            .queue_overlay_selection()
            .unwrap_or(0)
            .min(self.visible_queue_indices().len().saturating_sub(1));
        self.status = if self.visible_queue_indices().is_empty() {
            "Queue is empty.".to_string()
        } else {
            "Opened queue overlay.".to_string()
        };
    }

    fn close_queue_overlay(&mut self) {
        self.queue.overlay_visible = false;
        self.status = "Closed queue overlay.".to_string();
    }

    fn move_queue_selection(&mut self, down: bool) {
        let len = self.visible_queue_indices().len();
        if len == 0 {
            self.status = "Queue is empty.".to_string();
            return;
        }

        if down {
            self.queue.selected = self
                .queue
                .selected
                .saturating_add(1)
                .min(len.saturating_sub(1));
        } else {
            self.queue.selected = self.queue.selected.saturating_sub(1);
        }

        if let Some(row) = self
            .queue_overlay_rows()
            .get(self.queue.selected.min(len.saturating_sub(1)))
        {
            self.status = format!("Queued {} highlighted.", row.columns[0]);
        }
    }

    fn queue_selected_track(&mut self) {
        let Some(track) = self.selected_track_shortcut_target() else {
            self.status = "Select a track first.".to_string();
            return;
        };

        self.append_track_to_queue(track);
    }

    fn play_selected_queue_track(&mut self) {
        let indices = self.visible_queue_indices();
        let Some(plan_index) = indices
            .get(self.queue.selected.min(indices.len().saturating_sub(1)))
            .copied()
        else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        let Some(item) = self.playback_plan.items.get(plan_index).cloned() else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        self.playback_plan.current_index = Some(plan_index);
        self.start_track_playback(item.track, item.context);
    }

    fn remove_selected_queue_track(&mut self) {
        let indices = self.visible_queue_indices();
        let Some(plan_index) = indices
            .get(self.queue.selected.min(indices.len().saturating_sub(1)))
            .copied()
        else {
            self.status = "Queue is empty.".to_string();
            return;
        };

        if self.playback_plan.current_index == Some(plan_index) {
            self.status = "Can't remove the currently playing queued track.".to_string();
            return;
        }

        let removed = self.playback_plan.items.remove(plan_index);
        if let Some(current_index) = self.playback_plan.current_index.as_mut() {
            if plan_index < *current_index {
                *current_index = (*current_index).saturating_sub(1);
            }
        }

        let len = self.visible_queue_indices().len();
        self.queue.selected = self.queue.selected.min(len.saturating_sub(1));
        self.status = format!("Removed {} from the queue.", removed.track.title);
    }

    fn open_add_to_playlist_modal_for_selected_track(&mut self) {
        let track = self.selected_track_shortcut_target();
        self.open_add_to_playlist_modal(track, "Select a track first.");
    }

    fn open_add_to_playlist_modal_for_now_playing(&mut self) {
        let track = self.now_playing_shortcut_target();
        self.open_add_to_playlist_modal(track, "Nothing is playing right now.");
    }

    fn open_add_to_playlist_modal(
        &mut self,
        track: Option<TrackSummary>,
        missing_track_message: &str,
    ) {
        let Some(track) = track else {
            self.status = missing_track_message.to_string();
            return;
        };

        if self.session.is_none() {
            self.status = "Connect to SoundCloud before editing playlists.".to_string();
            return;
        }

        if self.playlists_loading && self.playlists.is_empty() {
            self.status = "Playlists are still loading. Try again in a moment.".to_string();
            return;
        }

        if self.playlists.is_empty() {
            if !self.playlists_loaded || self.playlists_error.is_some() {
                self.invalidate_playlists_sidebar();
                self.status = "Loading playlists before opening the playlist picker...".to_string();
            } else {
                self.status = "No playlists are available for this account yet.".to_string();
            }
            return;
        }

        self.add_to_playlist_modal = Some(AddToPlaylistModal {
            track: track.clone(),
            selected_playlist: self
                .selected_playlist
                .min(self.playlists.len().saturating_sub(1)),
        });
        self.status = format!("Choose a playlist for {}.", track.title);
    }

    fn like_selected_track(&mut self) {
        let track = self.selected_track_shortcut_target();
        self.like_track(track, "Select a track first.");
    }

    fn like_now_playing_track(&mut self) {
        let track = self.now_playing_shortcut_target();
        self.like_track(track, "Nothing is playing right now.");
    }

    fn like_track(&mut self, track: Option<TrackSummary>, missing_track_message: &str) {
        let Some(track) = track else {
            self.status = missing_track_message.to_string();
            return;
        };

        let Some(session) = self.session.clone() else {
            self.status = "Connect to SoundCloud before liking tracks.".to_string();
            return;
        };

        self.status = format!("Adding {} to Liked Songs...", track.title);
        self.queue_command(AppCommand::LikeTrack { session, track });
    }

    fn handle_playback_key(&mut self, key: KeyEvent) -> bool {
        let seek_seconds = self.settings.seek_duration_ms as f64 / 1000.0;
        let volume_increment = self.settings.volume_increment as f64;

        if self.settings.key_matches(KeyAction::TogglePlayback, key) {
            self.apply_playback_intent(PlaybackIntent::TogglePause);
            return true;
        }

        if self.settings.key_matches(KeyAction::NextTrack, key) {
            self.apply_playback_intent(PlaybackIntent::Next);
            return true;
        }

        if self.settings.key_matches(KeyAction::PreviousTrack, key) {
            self.apply_playback_intent(PlaybackIntent::Previous);
            return true;
        }

        if self
            .settings
            .key_matches(KeyAction::ForcePreviousTrack, key)
        {
            self.force_previous_track();
            return true;
        }

        if self.settings.key_matches(KeyAction::SeekBackwards, key) {
            self.apply_playback_intent(PlaybackIntent::SeekRelative {
                seconds: -seek_seconds,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::SeekForwards, key) {
            self.apply_playback_intent(PlaybackIntent::SeekRelative {
                seconds: seek_seconds,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::DecreaseVolume, key) {
            self.apply_playback_intent(PlaybackIntent::SetVolume {
                percent: self.player.volume_percent - volume_increment,
            });
            return true;
        }

        if self.settings.key_matches(KeyAction::IncreaseVolume, key) {
            self.apply_playback_intent(PlaybackIntent::SetVolume {
                percent: self.player.volume_percent + volume_increment,
            });
            return true;
        }

        false
    }

    fn handle_visualizer_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => self.cycle_visualizer_style(),
            (KeyCode::Char('v'), KeyModifiers::NONE) | (KeyCode::Esc, _) => self.close_visualizer(),
            _ if self.settings.key_matches(KeyAction::Back, key) => self.close_visualizer(),
            _ => {}
        }
    }

    fn queue_command(&mut self, command: AppCommand) {
        self.pending_commands.push_back(command);
    }

    fn toggle_visualizer(&mut self) {
        if self.visualizer.visible {
            self.close_visualizer();
        } else {
            self.open_visualizer();
        }
    }

    fn open_visualizer(&mut self) {
        if self.visualizer.visible {
            return;
        }

        self.show_welcome = false;
        self.visualizer.visible = true;
        self.visualizer.capture_active = false;
        self.visualizer.spectrum = SpectrumFrame::default();
        self.visualizer.status = "Starting system audio capture...".to_string();
        self.status = format!(
            "Opened visualizer in {} mode.",
            self.visualizer.style.label()
        );
        self.queue_command(AppCommand::ControlVisualizer(VisualizerCommand::Start));
    }

    fn close_visualizer(&mut self) {
        if !self.visualizer.visible {
            return;
        }

        self.visualizer.visible = false;
        self.visualizer.capture_active = false;
        self.status = "Closed visualizer.".to_string();
        self.queue_command(AppCommand::ControlVisualizer(VisualizerCommand::Stop));
    }

    fn cycle_visualizer_style(&mut self) {
        self.visualizer.style = self.visualizer.style.next();
        self.status = format!("Visualizer style set to {}.", self.visualizer.style.label());
    }

    fn sync_window_title(&mut self) {
        let title = if self.settings.set_window_title {
            match self.now_playing.track.as_ref() {
                Some(track) => format!("{} - {} | soundcloud-tui", track.title, track.artist),
                None => format!("{} | soundcloud-tui", self.route_title()),
            }
        } else {
            "soundcloud-tui".to_string()
        };

        self.queue_command(AppCommand::SetWindowTitle(title));
    }
    fn copy_now_playing_url(&mut self) {
        let Some(track) = self.now_playing.track.as_ref() else {
            self.show_main_error(
                "Could not copy share URL",
                "Nothing is playing right now, so there is no SoundCloud URL to copy.",
            );
            return;
        };

        let Some(url) = track.permalink_url.as_deref() else {
            self.show_main_error(
                "Could not copy share URL",
                format!("No SoundCloud URL is available for {}.", track.title),
            );
            return;
        };

        let label = track.title.clone();
        let text = url.to_string();

        self.queue_command(AppCommand::CopyText {
            text,
            label: label.clone(),
        });
        self.status = format!("Copying {} URL to the clipboard...", label);
    }

    fn cycle_repeat_mode(&mut self) {
        let next_mode = match self.player.repeat_mode {
            RepeatMode::Off => RepeatMode::Track,
            RepeatMode::Track => RepeatMode::Queue,
            RepeatMode::Queue => RepeatMode::Off,
        };
        self.set_repeat_mode(next_mode);
    }
    fn record_recent_playback(&mut self) {
        let Some(track) = self.now_playing.track.clone() else {
            return;
        };

        self.recent_history
            .record(track, self.now_playing.context.clone());
        self.queue_command(AppCommand::SaveHistory(self.recent_history.clone()));
    }
}
