impl AppState {
    pub fn on_tick(&mut self) {
        self.tick_count = self.tick_count.saturating_add(1);

        if self
            .toast
            .as_ref()
            .is_some_and(|toast| self.tick_count >= toast.expires_at_tick)
        {
            self.toast = None;
        }

        if let Some(loading) = &mut self.loading {
            loading.ticks_remaining = loading.ticks_remaining.saturating_sub(1);
            if loading.ticks_remaining == 0 {
                self.loading = None;
            }
        }
    }

    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.viewport = Viewport { width, height };
        self.help_scroll = self.max_help_scroll().min(self.help_scroll);
        self.status = match self.mode {
            AppMode::Auth => format!("Resized onboarding view to {}x{}.", width, height),
            AppMode::Main => format!(
                "Resized to {}x{} while focused on {}.",
                width,
                height,
                self.focus.label()
            ),
        };
    }

    pub fn set_loading(&mut self, message: impl Into<String>) {
        self.loading = Some(LoadingState {
            message: message.into(),
            ticks_remaining: 2,
        });
    }

    pub fn loading_label(&self) -> &str {
        self.loading
            .as_ref()
            .map(|loading| loading.message.as_str())
            .unwrap_or("Ready")
    }

    fn show_error_modal(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.add_to_playlist_modal = None;
        self.error_modal = Some(ErrorModal {
            title: title.into(),
            message: message.into(),
        });
    }

    fn show_main_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        let title = title.into();
        self.show_error_modal(title.clone(), message);
        self.status = title;
    }

    fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            expires_at_tick: self.tick_count.saturating_add(12),
        });
    }

    fn dismiss_error_modal(&mut self) {
        self.error_modal = None;
        self.status = "Dismissed the latest error.".to_string();
    }

    fn handle_error_modal_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter)
            || self.settings.key_matches(KeyAction::Back, key)
        {
            self.dismiss_error_modal();
        }
    }

    fn dismiss_add_to_playlist_modal(&mut self) {
        self.add_to_playlist_modal = None;
        self.status = "Cancelled add to playlist.".to_string();
    }

    fn handle_add_to_playlist_modal_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            _ if matches!(key.code, KeyCode::Esc)
                || self.settings.key_matches(KeyAction::Back, key) =>
            {
                self.dismiss_add_to_playlist_modal()
            }
            (KeyCode::Enter, _) => self.confirm_add_to_playlist_selection(),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.move_add_to_playlist_selection(1)
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.move_add_to_playlist_selection(-1)
            }
            (KeyCode::Char('H'), _) => self.jump_add_to_playlist_selection(0),
            (KeyCode::Char('M'), _) => {
                self.jump_add_to_playlist_selection(self.playlists.len().saturating_sub(1) / 2)
            }
            (KeyCode::Char('L'), _) => {
                self.jump_add_to_playlist_selection(self.playlists.len().saturating_sub(1))
            }
            _ => {}
        }
    }

    fn move_add_to_playlist_selection(&mut self, delta: isize) {
        let Some(current) = self
            .add_to_playlist_modal
            .as_ref()
            .map(|modal| modal.selected_playlist)
        else {
            return;
        };

        if self.playlists.is_empty() {
            self.status = "No playlists are available yet.".to_string();
            return;
        }

        let max_index = self.playlists.len().saturating_sub(1);
        let next = (current as isize + delta).clamp(0, max_index as isize) as usize;
        if let Some(modal) = self.add_to_playlist_modal.as_mut() {
            modal.selected_playlist = next;
        }

        if next == current && delta > 0 {
            let _ = self.maybe_queue_more_playlists();
        }

        if let Some(playlist) = self.playlists.get(next) {
            self.status = format!("Selected playlist {}.", playlist.title);
        }
    }

    fn jump_add_to_playlist_selection(&mut self, index: usize) {
        if self.playlists.is_empty() {
            self.status = "No playlists are available yet.".to_string();
            return;
        }

        let next = index.min(self.playlists.len().saturating_sub(1));
        if let Some(modal) = self.add_to_playlist_modal.as_mut() {
            modal.selected_playlist = next;
        }

        if let Some(playlist) = self.playlists.get(next) {
            self.status = format!("Selected playlist {}.", playlist.title);
        }
    }

    fn confirm_add_to_playlist_selection(&mut self) {
        let Some(modal) = self.add_to_playlist_modal.clone() else {
            return;
        };

        let Some(session) = self.session.clone() else {
            self.dismiss_add_to_playlist_modal();
            return;
        };

        let Some(playlist_urn) = self
            .playlists
            .get(modal.selected_playlist)
            .and_then(|playlist| playlist.urn.as_deref())
        else {
            self.status = "Select a playlist first.".to_string();
            return;
        };
        let Some(playlist) = self.known_playlists.get(playlist_urn).cloned() else {
            self.status = "The selected playlist details are not available yet.".to_string();
            return;
        };

        self.add_to_playlist_modal = None;
        self.status = format!("Adding {} to {}...", modal.track.title, playlist.title);
        self.queue_command(AppCommand::AddTrackToPlaylist {
            session,
            track: modal.track,
            playlist,
        });
    }

    fn max_help_scroll(&self) -> usize {
        self.help_row_count()
            .saturating_sub(self.help_visible_rows())
    }

    fn scroll_help(&mut self, delta: isize) {
        let next = self.help_scroll as isize + delta;
        self.help_scroll = next.clamp(0, self.max_help_scroll() as isize) as usize;
    }

    fn page_help(&mut self, down: bool) {
        let step = self.help_visible_rows().max(1) as isize;
        self.scroll_help(if down { step } else { -step });
    }

    fn content_page_size(&self) -> usize {
        self.viewport
            .height
            .saturating_sub(self.layout.playbar_height + 8)
            .max(6) as usize
    }

    fn playlists_page_size(&self) -> usize {
        self.viewport
            .height
            .saturating_sub(self.layout.playbar_height + self.layout.library_height + 8)
            .max(4) as usize
    }

    fn page_results(&mut self, down: bool) -> bool {
        match self.focus {
            Focus::Content => self.page_content(down),
            Focus::Playlists => self.page_playlists(down),
            _ => false,
        }
    }

    fn page_content(&mut self, down: bool) -> bool {
        let len = self.current_content_len();
        if len == 0 {
            return down && self.maybe_queue_current_route_next_page();
        }

        let step = self.content_page_size();
        let max_index = len.saturating_sub(1);
        let next = if down {
            self.selected_content.saturating_add(step).min(max_index)
        } else {
            self.selected_content.saturating_sub(step)
        };
        let moved = next != self.selected_content;
        self.selected_content = next;

        if moved {
            if let Some(label) = self.current_selection_label() {
                self.status = format!("Highlighted {}.", label);
            }
        }

        let queued_more = down
            && self.selected_content == max_index
            && self.maybe_queue_current_route_next_page();
        moved || queued_more
    }

    fn page_playlists(&mut self, down: bool) -> bool {
        if self.playlists.is_empty() {
            return down && self.maybe_queue_more_playlists();
        }

        let step = self.playlists_page_size();
        let max_index = self.playlists.len().saturating_sub(1);
        let next = if down {
            self.selected_playlist.saturating_add(step).min(max_index)
        } else {
            self.selected_playlist.saturating_sub(step)
        };
        let moved = next != self.selected_playlist;
        self.selected_playlist = next;

        if moved {
            self.sync_route_from_playlist();
        }

        let queued_more =
            down && self.selected_playlist == max_index && self.maybe_queue_more_playlists();
        moved || queued_more
    }

    fn open_settings_menu(&mut self) {
        self.show_help = false;
        self.settings_menu = Some(SettingsMenuState::new(&self.settings));
        self.status = "Opened settings.".to_string();
    }

    fn close_settings_menu(&mut self) {
        let discarded = self
            .settings_menu
            .as_ref()
            .map(|menu| menu.has_unsaved_changes(&self.settings))
            .unwrap_or(false);
        self.settings_menu = None;
        self.status = if discarded {
            "Discarded unsaved settings changes.".to_string()
        } else {
            "Closed settings.".to_string()
        };
    }

    fn save_settings_menu(&mut self) {
        let Some(mut menu) = self.settings_menu.take() else {
            return;
        };

        let previous = self.settings.clone();
        menu.draft.normalize();
        if let Err(error) = menu.draft.validate() {
            self.show_main_error("Could not save settings", error.to_string());
            self.settings_menu = Some(menu);
            return;
        }

        self.settings = menu.draft.clone();
        self.queue_command(AppCommand::SaveSettings(self.settings.clone()));
        self.apply_runtime_settings(&previous);
        menu.draft = self.settings.clone();
        menu.editing = false;
        menu.edit_buffer.clear();
        self.settings_menu = Some(menu);

        let restart_note = if previous.tick_rate_ms != self.settings.tick_rate_ms {
            " Tick rate applies on the next launch."
        } else {
            ""
        };
        self.status = format!("Saved settings.{}", restart_note);
    }

    fn handle_settings_key(&mut self, key: KeyEvent) {
        let Some(mut menu) = self.settings_menu.take() else {
            return;
        };

        if menu.editing {
            let selected = menu.items().get(menu.selected_index()).cloned();
            if matches!(key.code, KeyCode::Esc) {
                menu.cancel_edit();
                self.status = "Cancelled the current settings edit.".to_string();
                self.settings_menu = Some(menu);
                return;
            }

            match selected.map(|item| item.value) {
                Some(crate::app::SettingsValue::Key(_)) => match menu.capture_keybinding(key) {
                    Ok(binding) => {
                        self.status = format!("Bound setting to {}.", binding);
                    }
                    Err(error) => {
                        self.show_main_error("Could not update keybinding", error.to_string());
                    }
                },
                Some(crate::app::SettingsValue::Number(_))
                | Some(crate::app::SettingsValue::Text(_))
                | Some(crate::app::SettingsValue::Color(_)) => match (key.code, key.modifiers) {
                    (KeyCode::Enter, _) => match menu.confirm_edit() {
                        Ok(()) => self.status = "Updated the draft setting value.".to_string(),
                        Err(error) => {
                            self.show_main_error("Could not update setting", error.to_string())
                        }
                    },
                    (KeyCode::Backspace, _) => {
                        menu.edit_buffer.pop();
                    }
                    (KeyCode::Char(ch), modifiers)
                        if modifiers.intersection(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            == KeyModifiers::NONE =>
                    {
                        menu.edit_buffer.push(ch);
                    }
                    _ => {}
                },
                _ => {}
            }

            self.settings_menu = Some(menu);
            return;
        }

        if self.settings.key_matches(KeyAction::SaveSettings, key) {
            self.settings_menu = Some(menu);
            self.save_settings_menu();
            return;
        }

        if matches!(key.code, KeyCode::Esc) || self.settings.key_matches(KeyAction::Back, key) {
            self.settings_menu = Some(menu);
            self.close_settings_menu();
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Left, _) => menu.switch_tab(false),
            (KeyCode::Right, _) => menu.switch_tab(true),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => menu.move_selection(1),
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => menu.move_selection(-1),
            (KeyCode::Enter, _) => match menu.activate_selected() {
                Ok(_) => {
                    self.status = format!("Editing {} settings.", menu.tab.label());
                }
                Err(error) => self.show_main_error("Could not update setting", error.to_string()),
            },
            _ => {}
        }

        self.settings_menu = Some(menu);
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        if self.mode != AppMode::Main {
            return;
        }

        if self.visualizer.visible {
            return;
        }

        if self.error_modal.is_some() {
            self.handle_error_modal_mouse(mouse);
            return;
        }

        if self.show_settings() {
            self.handle_settings_mouse(mouse);
            return;
        }

        if self.show_help {
            self.handle_help_mouse(mouse);
            return;
        }

        if self.queue.overlay_visible {
            self.handle_queue_mouse(mouse);
            return;
        }

        if self.add_to_playlist_modal.is_some() {
            self.handle_add_to_playlist_mouse(mouse);
            return;
        }

        if self.show_welcome {
            self.show_welcome = false;
            self.status = "Closed welcome overlay.".to_string();
        }

        self.handle_main_mouse(mouse);
    }

    fn register_click(&mut self, target: MouseClickTarget) -> bool {
        let now = Instant::now();
        let is_double_click = self.last_mouse_click.is_some_and(|previous| {
            previous.target == target && now.duration_since(previous.at) <= DOUBLE_CLICK_WINDOW
        });
        self.last_mouse_click = Some(MouseClickState { target, at: now });
        is_double_click
    }

    fn focus_main_pane(&mut self, focus: Focus) {
        self.set_focus(focus);
        self.status = format!("Focused {}.", focus.label());
    }

    fn handle_error_modal_mouse(&mut self, mouse: MouseEvent) {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            self.dismiss_error_modal();
        }
    }

    fn handle_settings_mouse(&mut self, mouse: MouseEvent) {
        let Some(area) = geometry::viewport_area(self) else {
            return;
        };
        let layout = geometry::settings_layout(area);
        if !rect_contains(layout.overlay, mouse.column, mouse.row) {
            return;
        }

        let Some(mut menu) = self.settings_menu.take() else {
            return;
        };

        if menu.editing {
            self.settings_menu = Some(menu);
            return;
        }

        if let Some(delta) = mouse_scroll_delta(mouse.kind) {
            if rect_contains(layout.list, mouse.column, mouse.row) {
                menu.move_selection(delta);
            }
            self.settings_menu = Some(menu);
            return;
        }

        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            if rect_contains(layout.tabs, mouse.column, mouse.row) {
                if let Some(tab) = geometry::settings_tab_at(layout.tabs, mouse.column, mouse.row) {
                    menu.select_tab(tab);
                }
            } else if rect_contains(layout.list, mouse.column, mouse.row) {
                let items = menu.items();
                if let Some(index) = block_list_index_at_row(
                    layout.list,
                    mouse.column,
                    mouse.row,
                    items.len(),
                    menu.selected_index(),
                ) {
                    let was_selected = index == menu.selected_index();
                    menu.set_selected_index(index);

                    if was_selected {
                        match menu.activate_selected() {
                            Ok(_) => {
                                self.status = format!("Editing {} settings.", menu.tab.label());
                            }
                            Err(error) => {
                                self.show_main_error("Could not update setting", error.to_string());
                            }
                        }
                    }
                }
            }
        }

        self.settings_menu = Some(menu);
    }

    fn handle_help_mouse(&mut self, mouse: MouseEvent) {
        let Some(area) = geometry::viewport_area(self) else {
            return;
        };
        let layout = geometry::help_layout(area);
        if !rect_contains(layout.overlay, mouse.column, mouse.row) {
            return;
        }

        if let Some(delta) = mouse_scroll_delta(mouse.kind) {
            if rect_contains(layout.body, mouse.column, mouse.row) {
                self.scroll_help(delta);
            }
        }
    }

    fn handle_queue_mouse(&mut self, mouse: MouseEvent) {
        let Some(area) = geometry::viewport_area(self) else {
            return;
        };
        let layout = geometry::queue_layout(area);
        if !rect_contains(layout.overlay, mouse.column, mouse.row) {
            return;
        }

        if let Some(delta) = mouse_scroll_delta(mouse.kind) {
            if rect_contains(layout.body, mouse.column, mouse.row) {
                self.move_queue_selection(delta > 0);
            }
            return;
        }

        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            || !rect_contains(layout.body, mouse.column, mouse.row)
        {
            return;
        }

        let rows = self.queue_overlay_rows();
        if let Some(index) = table_index_at_row(
            layout.body,
            mouse.column,
            mouse.row,
            rows.len(),
            self.queue.selected,
        ) {
            let was_selected = index == self.queue.selected;
            self.queue.selected = index;

            if was_selected {
                self.play_selected_queue_track();
            } else if let Some(row) = rows.get(index) {
                self.status = format!("Queued {} highlighted.", row.columns[0]);
            }
        }
    }

    fn handle_add_to_playlist_mouse(&mut self, mouse: MouseEvent) {
        let Some(area) = geometry::viewport_area(self) else {
            return;
        };
        let layout = geometry::add_to_playlist_layout(area);
        if !rect_contains(layout.overlay, mouse.column, mouse.row) {
            return;
        }

        if let Some(delta) = mouse_scroll_delta(mouse.kind) {
            if rect_contains(layout.list, mouse.column, mouse.row) {
                self.move_add_to_playlist_selection(delta);
            }
            return;
        }

        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            || !rect_contains(layout.list, mouse.column, mouse.row)
        {
            return;
        }

        let Some(current) = self
            .add_to_playlist_modal
            .as_ref()
            .map(|modal| modal.selected_playlist)
        else {
            return;
        };

        if let Some(index) = plain_list_index_at_row(
            layout.list,
            mouse.column,
            mouse.row,
            self.playlists.len(),
            current,
        ) {
            let was_selected = index == current;
            if let Some(modal) = self.add_to_playlist_modal.as_mut() {
                modal.selected_playlist = index;
            }

            if was_selected {
                self.confirm_add_to_playlist_selection();
            } else if let Some(playlist) = self.playlists.get(index) {
                self.status = format!("Selected playlist {}.", playlist.title);
            }
        }
    }

    fn handle_main_mouse(&mut self, mouse: MouseEvent) {
        let Some(layout) = geometry::main_layout_from_viewport(self) else {
            return;
        };

        if let Some(delta) = mouse_scroll_delta(mouse.kind) {
            if rect_contains(layout.library, mouse.column, mouse.row) {
                self.focus_main_pane(Focus::Library);
                self.apply(if delta > 0 {
                    Action::MoveDown
                } else {
                    Action::MoveUp
                });
            } else if rect_contains(layout.playlists, mouse.column, mouse.row) {
                self.focus_main_pane(Focus::Playlists);
                self.apply(if delta > 0 {
                    Action::MoveDown
                } else {
                    Action::MoveUp
                });
            } else if rect_contains(layout.content, mouse.column, mouse.row) {
                self.focus_main_pane(Focus::Content);
                self.apply(if delta > 0 {
                    Action::MoveDown
                } else {
                    Action::MoveUp
                });
            }
            return;
        }

        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return;
        }

        if rect_contains(layout.search, mouse.column, mouse.row) {
            self.begin_search_input();
            return;
        }

        if rect_contains(layout.help, mouse.column, mouse.row) {
            self.help_scroll = 0;
            self.show_help = true;
            self.status = "Showing help menu.".to_string();
            return;
        }

        if rect_contains(layout.settings, mouse.column, mouse.row) {
            self.open_settings_menu();
            return;
        }

        if let Some(index) = block_list_index_at_row(
            layout.library,
            mouse.column,
            mouse.row,
            self.library_items.len(),
            self.selected_library,
        ) {
            self.focus_main_pane(Focus::Library);
            self.selected_library = index;
            self.sync_route_from_library();
            return;
        }

        if rect_contains(layout.library, mouse.column, mouse.row) {
            self.focus_main_pane(Focus::Library);
            return;
        }

        if let Some(index) = block_list_index_at_row(
            layout.playlists,
            mouse.column,
            mouse.row,
            self.playlists.len(),
            self.selected_playlist,
        ) {
            self.focus_main_pane(Focus::Playlists);
            self.selected_playlist = index;
            self.sync_route_from_playlist();
            return;
        }

        if rect_contains(layout.playlists, mouse.column, mouse.row) {
            self.focus_main_pane(Focus::Playlists);
            return;
        }

        let content_layout = geometry::content_layout(layout.content, self);
        if let Some(index) = table_index_at_row(
            content_layout.body,
            mouse.column,
            mouse.row,
            self.current_content_len(),
            self.selected_content,
        ) {
            let activate_selected =
                self.register_click(MouseClickTarget::ContentRow(self.route, index));
            self.focus_main_pane(Focus::Content);
            self.selected_content = index;
            if activate_selected {
                self.select_current_content();
            } else if let Some(label) = self.current_selection_label() {
                self.status = format!("Highlighted {}.", label);
            }
            return;
        }

        if rect_contains(layout.content, mouse.column, mouse.row) {
            self.focus_main_pane(Focus::Content);
            return;
        }

        if rect_contains(layout.playbar, mouse.column, mouse.row) {
            self.focus_main_pane(Focus::Playbar);
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if is_global_quit_key(key) {
            self.should_quit = true;
            return;
        }

        match self.mode {
            AppMode::Auth => {
                if let Some(intent) = self.auth.handle_key(key) {
                    self.handle_auth_intent(intent);
                }
            }
            AppMode::Main => {
                if self.visualizer.visible {
                    self.handle_visualizer_key(key);
                    return;
                }

                if self.show_help {
                    self.handle_help_key(key);
                    return;
                }

                if self.error_modal.is_some() {
                    self.handle_error_modal_key(key);
                    return;
                }

                if self.settings_menu.is_some() {
                    self.handle_settings_key(key);
                    return;
                }

                if self.add_to_playlist_modal.is_some() {
                    self.handle_add_to_playlist_modal_key(key);
                    return;
                }

                if self.queue.overlay_visible {
                    self.handle_queue_key(key);
                    return;
                }

                if self.show_welcome {
                    self.show_welcome = false;
                }

                if self.focus == Focus::Search && self.handle_search_key(key) {
                    return;
                }

                if self.handle_main_shortcut_key(key) {
                    return;
                }

                if self.handle_route_key(key) {
                    return;
                }

                if self.handle_playback_key(key) {
                    return;
                }

                if let Some(action) = map_main_key_event(key) {
                    self.apply(action);
                }
            }
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::F(1))
            || self.settings.key_matches(KeyAction::Help, key)
            || self.settings.key_matches(KeyAction::Back, key)
        {
            self.dismiss_help();
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => self.scroll_help(1),
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => self.scroll_help(-1),
            _ if self.settings.key_matches(KeyAction::NextPage, key) => self.page_help(true),
            _ if self.settings.key_matches(KeyAction::PreviousPage, key) => self.page_help(false),
            _ => {}
        }
    }

    fn handle_queue_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            _ if matches!(key.code, KeyCode::Esc)
                || self.settings.key_matches(KeyAction::Back, key) =>
            {
                self.close_queue_overlay()
            }
            (KeyCode::Enter, _) => self.play_selected_queue_track(),
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                self.move_queue_selection(true)
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                self.move_queue_selection(false)
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => self.remove_selected_queue_track(),
            _ => {}
        }
    }

    fn handle_main_shortcut_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Char('v'), KeyModifiers::NONE) => {
                self.toggle_visualizer();
                return true;
            }
            (KeyCode::Char('V'), KeyModifiers::SHIFT) => {
                self.cycle_visualizer_style();
                return true;
            }
            _ => {}
        }

        if self.settings.key_matches(KeyAction::Search, key) {
            self.begin_search_input();
            return true;
        }

        if self.settings.key_matches(KeyAction::AddToQueue, key) && self.focus == Focus::Content {
            self.queue_selected_track();
            return true;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('w'), KeyModifiers::NONE) if self.focus == Focus::Content => {
                self.open_add_to_playlist_modal_for_selected_track();
                true
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) if self.focus == Focus::Content => {
                self.like_selected_track();
                true
            }
            (KeyCode::Char('W'), KeyModifiers::SHIFT) => {
                self.open_add_to_playlist_modal_for_now_playing();
                true
            }
            (KeyCode::Char('L'), KeyModifiers::SHIFT) => {
                self.like_now_playing_track();
                true
            }
            (KeyCode::F(1), _) => {
                self.help_scroll = 0;
                self.show_help = true;
                self.status = "Showing help menu.".to_string();
                true
            }
            (KeyCode::Esc, _) if self.focus == Focus::Content => {
                self.focus = self.content_return_focus;
                self.status = format!("Returned focus to {}.", self.focus.label());
                true
            }
            (KeyCode::Char('{'), _) => {
                self.adjust_sidebar_width(-2);
                true
            }
            (KeyCode::Char('}'), _) => {
                self.adjust_sidebar_width(2);
                true
            }
            (KeyCode::Char('('), _) => {
                self.adjust_primary_panel_height(-1);
                true
            }
            (KeyCode::Char(')'), _) => {
                self.adjust_primary_panel_height(1);
                true
            }
            (KeyCode::Char('|'), _) => {
                self.reset_layout();
                true
            }
            (KeyCode::F(5), _) => {
                self.reload_current_route();
                true
            }
            _ if self.settings.key_matches(KeyAction::ShowQueue, key) => {
                self.open_queue_overlay();
                true
            }
            _ if self.settings.key_matches(KeyAction::Help, key) => {
                self.help_scroll = 0;
                self.show_help = true;
                self.status = "Showing help menu.".to_string();
                true
            }
            _ if self.settings.key_matches(KeyAction::OpenSettings, key) => {
                self.open_settings_menu();
                true
            }
            _ if self.settings.key_matches(KeyAction::NextPage, key) => self.page_results(true),
            _ if self.settings.key_matches(KeyAction::PreviousPage, key) => {
                self.page_results(false)
            }
            _ if self.settings.key_matches(KeyAction::Repeat, key) => {
                self.cycle_repeat_mode();
                true
            }
            _ if self.settings.key_matches(KeyAction::Shuffle, key) => {
                self.apply_playback_intent(PlaybackIntent::SetShuffle(
                    !self.player.shuffle_enabled,
                ));
                true
            }
            _ if self.settings.key_matches(KeyAction::CopySongUrl, key) => {
                self.copy_now_playing_url();
                true
            }
            _ => false,
        }
    }

    fn handle_route_key(&mut self, key: KeyEvent) -> bool {
        match self.route {
            Route::Search => match key.code {
                KeyCode::Char('1') => {
                    self.set_search_view(SearchView::Tracks);
                    true
                }
                KeyCode::Char('2') => {
                    self.set_search_view(SearchView::Playlists);
                    true
                }
                KeyCode::Char('3') => {
                    self.set_search_view(SearchView::Users);
                    true
                }
                _ => false,
            },
            Route::UserProfile => match key.code {
                KeyCode::Char('1') => {
                    self.set_user_profile_view(UserProfileView::Tracks);
                    true
                }
                KeyCode::Char('2') => {
                    self.set_user_profile_view(UserProfileView::Playlists);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn handle_paste_event(&mut self, text: &str) {
        match self.mode {
            AppMode::Auth => {
                self.auth.paste_text(text);
                self.status = "Pasted clipboard contents into the active field.".to_string();
            }
            AppMode::Main if self.settings_menu.as_ref().is_some_and(|menu| menu.editing) => {
                if let Some(menu) = self.settings_menu.as_mut() {
                    let sanitized = text.replace(['\r', '\n'], " ");
                    menu.edit_buffer.push_str(sanitized.trim());
                    self.status = "Pasted text into the current settings field.".to_string();
                }
            }
            AppMode::Main if self.focus == Focus::Search => {
                self.show_welcome = false;
                let sanitized = text.replace(['\r', '\n'], " ");
                self.insert_search_text(sanitized.trim());
                self.status = format!("Updated search query to '{}'.", self.search_query);
            }
            AppMode::Main => {
                self.show_welcome = false;
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.focus = self.search_return_focus;
                self.status = "Closed search input.".to_string();
                true
            }
            (KeyCode::Enter, _) => {
                self.submit_search();
                true
            }
            (KeyCode::Left, _) => {
                self.search_cursor = self.search_cursor.saturating_sub(1);
                true
            }
            (KeyCode::Right, _) => {
                self.search_cursor =
                    (self.search_cursor + 1).min(self.search_query.chars().count());
                true
            }
            (KeyCode::Home, _) | (KeyCode::Char('i'), KeyModifiers::CONTROL) => {
                self.search_cursor = 0;
                true
            }
            (KeyCode::End, _) | (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.search_cursor = self.search_query.chars().count();
                true
            }
            (KeyCode::Backspace, _) => {
                self.backspace_search();
                true
            }
            (KeyCode::Delete, _) => {
                self.delete_search();
                true
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.search_query.clear();
                self.search_cursor = 0;
                self.status = "Cleared the search query.".to_string();
                true
            }
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.delete_search_to_start();
                true
            }
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                self.delete_search_to_end();
                true
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.delete_previous_word();
                true
            }
            (KeyCode::Char(ch), KeyModifiers::NONE) | (KeyCode::Char(ch), KeyModifiers::SHIFT) => {
                self.insert_search_char(ch);
                true
            }
            _ => false,
        }
    }

    fn begin_search_input(&mut self) {
        if self.focus != Focus::Search {
            self.search_return_focus = self.focus;
        }
        self.set_focus(Focus::Search);
        self.search_cursor = self.search_query.chars().count();
        self.status = "Editing search query. Press Enter to search.".to_string();
    }

    fn handle_auth_intent(&mut self, intent: AuthIntent) {
        match intent {
            AuthIntent::OpenAppsPage => {
                self.status = "Opening SoundCloud app registration in your browser...".to_string();
                self.queue_command(AppCommand::OpenUrl(
                    "https://soundcloud.com/you/apps".to_string(),
                ));
            }
            AuthIntent::SaveAndContinue => {
                let credentials = self.auth.credentials();
                match credentials.validate() {
                    Ok(()) => match crate::soundcloud::auth::prepare_authorization(credentials) {
                        Ok(request) => {
                            self.auth.clear_error();
                            self.auth.set_info(
                                "Saving your SoundCloud app credentials locally before opening the browser.",
                            );
                            self.set_loading("Saving SoundCloud credentials locally...");
                            self.status =
                                "Saving your SoundCloud credentials locally...".to_string();
                            self.queue_command(AppCommand::SaveCredentials(request));
                        }
                        Err(error) => {
                            self.auth.set_error(error.to_string());
                            self.status = error.to_string();
                        }
                    },
                    Err(error) => {
                        self.auth.set_error(error.to_string());
                        self.status = error.to_string();
                    }
                }
            }
            AuthIntent::OpenBrowser => {
                if let Some(url) = &self.auth.auth_url {
                    self.status =
                        "Opening the SoundCloud authorize page in your browser...".to_string();
                    self.queue_command(AppCommand::OpenUrl(url.clone()));
                }
            }
            AuthIntent::ShowManualCallback => {
                self.auth.show_manual_callback(
                    "Paste the full callback URL from your browser after approving SoundCloud access.",
                );
                self.status = "Waiting for manual callback URL entry.".to_string();
            }
            AuthIntent::BackToCredentials => {
                self.auth.back_to_credentials();
                self.loading = None;
                self.status = "Edit your credentials and try again.".to_string();
            }
            AuthIntent::SubmitManualCallback => {
                if let Some(request) = self.auth.pending_authorization.clone() {
                    self.auth.clear_error();
                    self.set_loading("Submitting the pasted callback URL...");
                    self.queue_command(AppCommand::ExchangeAuthorizationCode {
                        request,
                        callback_input: self.auth.callback_input.value.clone(),
                    });
                }
            }
            AuthIntent::BackToBrowser => {
                self.auth.step = crate::app::AuthStep::WaitingForBrowser;
                self.auth.focus = crate::app::AuthFocus::OpenBrowser;
                self.status = "Waiting for the browser callback again.".to_string();
            }
        }
    }

    fn complete_auth(&mut self, session: AuthorizedSession) {
        self.mode = AppMode::Main;
        self.loading = None;
        self.session = Some(session.clone());
        self.set_auth_session(&session);
        self.reset_live_data();
        self.apply_startup_behavior();
        self.show_welcome = !self.settings.show_help_on_startup;
        if self.settings.show_help_on_startup {
            self.help_scroll = 0;
            self.show_help = true;
            self.help_requires_acknowledgement = true;
            self.status =
                "Connected successfully. Review the help menu for first-run guidance.".to_string();
        }
        self.request_playlists_load(false);
        self.request_route_load(false);
        self.sync_window_title();
    }

    fn apply_runtime_settings(&mut self, previous: &Settings) {
        if !self.settings.draw_cover_art {
            self.cover_art = CoverArt::default();
        }

        if previous.startup_behavior != self.settings.startup_behavior && self.session.is_some() {
            self.apply_startup_behavior();
        }

        self.sync_window_title();
    }

    fn apply_startup_behavior(&mut self) {
        let Some(track) = self
            .recent_history
            .entries
            .first()
            .map(|entry| entry.track.clone())
        else {
            return;
        };

        match self.settings.startup_behavior {
            StartupBehavior::Continue => {}
            StartupBehavior::Pause => {
                self.now_playing = NowPlaying {
                    track: Some(track.clone()),
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    context: "Startup: recent track".to_string(),
                    artwork_url: track.artwork_url.clone(),
                    elapsed_label: "0:00".to_string(),
                    duration_label: track.duration_label(),
                    progress_ratio: 0.0,
                };
                self.refresh_cover_art(track.artwork_url.as_deref());
                self.status = format!("Loaded {} into the playbar.", track.title);
            }
            StartupBehavior::Play => {
                self.now_playing.context = "Startup: recent track".to_string();
                self.start_track_playback(track.clone(), "Startup: recent track".to_string());
                self.status = format!("Starting your most recent track: {}.", track.title);
            }
        }
    }

    fn dismiss_help(&mut self) {
        self.show_help = false;

        if self.help_requires_acknowledgement {
            self.help_requires_acknowledgement = false;
            if self.settings.show_help_on_startup {
                self.settings.show_help_on_startup = false;
                self.queue_command(AppCommand::SaveSettings(self.settings.clone()));
            }
            self.status = "Help dismissed. You can reopen it anytime with ?.".to_string();
        } else {
            self.status = "Help closed.".to_string();
        }
    }

    fn adjust_sidebar_width(&mut self, delta: i16) {
        let next = (self.layout.sidebar_width_percent as i16 + delta).clamp(14, 40) as u16;
        self.layout.sidebar_width_percent = next;
        self.status = format!("Sidebar width set to {}%.", next);
    }

    fn adjust_primary_panel_height(&mut self, delta: i16) {
        match self.focus {
            Focus::Library => {
                let next = (self.layout.library_height as i16 + delta).clamp(4, 18) as u16;
                self.layout.library_height = next;
                self.status = format!("Library height set to {} rows.", next);
            }
            _ => {
                let next = (self.layout.playbar_height as i16 + delta).clamp(4, 12) as u16;
                self.layout.playbar_height = next;
                self.status = format!("Playbar height set to {} rows.", next);
            }
        }
    }

    fn reset_layout(&mut self) {
        self.layout = LayoutState::default();
        self.status = "Layout reset to defaults.".to_string();
    }
    fn submit_search(&mut self) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.status = "Enter a search query first.".to_string();
            return;
        }

        self.search_query = query;
        self.search_cursor = self.search_query.chars().count();
        self.focus_content_from(self.search_return_focus);
        self.route = Route::Search;
        self.search_view = SearchView::Tracks;
        self.selected_content = 0;
        self.status = format!("Searching SoundCloud for '{}'...", self.search_query);
        self.request_route_load(false);
    }

    fn insert_search_char(&mut self, ch: char) {
        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        chars.insert(self.search_cursor, ch);
        self.search_query = chars.into_iter().collect();
        self.search_cursor += 1;
    }

    fn insert_search_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_search_char(ch);
        }
    }

    fn delete_search_to_start(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let chars = self.search_query.chars().collect::<Vec<_>>();
        self.search_query = chars[self.search_cursor..].iter().copied().collect();
        self.search_cursor = 0;
        self.status = "Deleted text before the cursor.".to_string();
    }

    fn delete_search_to_end(&mut self) {
        let chars = self.search_query.chars().collect::<Vec<_>>();
        if self.search_cursor >= chars.len() {
            return;
        }

        self.search_query = chars[..self.search_cursor].iter().copied().collect();
        self.status = "Deleted text after the cursor.".to_string();
    }

    fn delete_previous_word(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let chars = self.search_query.chars().collect::<Vec<_>>();
        let mut word_start = self.search_cursor;

        while word_start > 0 && chars[word_start - 1].is_whitespace() {
            word_start -= 1;
        }
        while word_start > 0 && !chars[word_start - 1].is_whitespace() {
            word_start -= 1;
        }

        let mut updated = chars[..word_start].to_vec();
        updated.extend_from_slice(&chars[self.search_cursor..]);
        self.search_query = updated.into_iter().collect();
        self.search_cursor = word_start;
        self.status = "Deleted the previous word.".to_string();
    }

    fn backspace_search(&mut self) {
        if self.search_cursor == 0 {
            return;
        }

        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        chars.remove(self.search_cursor - 1);
        self.search_query = chars.into_iter().collect();
        self.search_cursor -= 1;
    }

    fn delete_search(&mut self) {
        let mut chars = self.search_query.chars().collect::<Vec<_>>();
        if self.search_cursor >= chars.len() {
            return;
        }

        chars.remove(self.search_cursor);
        self.search_query = chars.into_iter().collect();
    }
}
