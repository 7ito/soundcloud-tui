use anyhow::{Result, anyhow};
use crossterm::event::KeyEvent;

use crate::config::settings::{
    KeyAction, Settings, StartupBehavior, event_to_keybinding, normalize_keybinding, parse_color,
    theme_preset_names,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SettingsTab {
    Behavior,
    Keybindings,
    Theme,
}

impl SettingsTab {
    pub const ALL: [Self; 3] = [Self::Behavior, Self::Keybindings, Self::Theme];

    pub fn label(self) -> &'static str {
        match self {
            Self::Behavior => "Behavior",
            Self::Keybindings => "Keybindings",
            Self::Theme => "Theme",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Behavior => Self::Keybindings,
            Self::Keybindings => Self::Theme,
            Self::Theme => Self::Behavior,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            Self::Behavior => Self::Theme,
            Self::Keybindings => Self::Behavior,
            Self::Theme => Self::Keybindings,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Behavior => 0,
            Self::Keybindings => 1,
            Self::Theme => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SettingField {
    Behavior(BehaviorField),
    Keybinding(KeyAction),
    ThemePreset,
    ThemeColor(ThemeColorField),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BehaviorField {
    SeekDuration,
    VolumeIncrement,
    TickRate,
    TextEmphasis,
    LoadingIndicator,
    WideSearchBar,
    SetWindowTitle,
    StopAfterCurrentTrack,
    StartupBehavior,
    LikedIcon,
    ShuffleIcon,
    PlayingIcon,
    PausedIcon,
    DrawCoverArt,
    ForceDrawCoverArt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThemeColorField {
    Active,
    Banner,
    Hint,
    Hovered,
    Selected,
    Inactive,
    Text,
    ErrorText,
    PlaybarBackground,
    PlaybarProgress,
    LyricsHighlight,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SettingsValue {
    Bool(bool),
    Number(i64),
    Text(String),
    Key(String),
    Color(String),
    Cycle {
        current: String,
        options: &'static [&'static str],
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SettingsItem {
    pub field: SettingField,
    pub name: &'static str,
    pub value: SettingsValue,
}

impl SettingsItem {
    pub fn display_value(&self) -> String {
        match &self.value {
            SettingsValue::Bool(enabled) => {
                if *enabled {
                    "[o] On".to_string()
                } else {
                    "[o] Off".to_string()
                }
            }
            SettingsValue::Number(value) => value.to_string(),
            SettingsValue::Text(value) => format!("\"{value}\""),
            SettingsValue::Key(value) => format!("[{value}]"),
            SettingsValue::Color(value) => format!("■ {value}"),
            SettingsValue::Cycle { current, .. } => format!("◆ {current} ◆"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsMenuState {
    pub tab: SettingsTab,
    pub editing: bool,
    pub edit_buffer: String,
    pub draft: Settings,
    selected: [usize; 3],
}

pub enum ActivateResult {
    Changed,
    EditingStarted,
}

impl SettingsMenuState {
    pub fn new(settings: &Settings) -> Self {
        Self {
            tab: SettingsTab::Behavior,
            editing: false,
            edit_buffer: String::new(),
            draft: settings.clone(),
            selected: [0, 0, 0],
        }
    }

    pub fn items(&self) -> Vec<SettingsItem> {
        match self.tab {
            SettingsTab::Behavior => behavior_items(&self.draft),
            SettingsTab::Keybindings => keybinding_items(&self.draft),
            SettingsTab::Theme => theme_items(&self.draft),
        }
    }

    pub fn selected_index(&self) -> usize {
        self.selected[self.tab.index()]
    }

    pub fn set_selected_index(&mut self, index: usize) {
        self.selected[self.tab.index()] = index;
    }

    pub fn move_selection(&mut self, delta: isize) {
        let len = self.items().len();
        if len == 0 {
            self.set_selected_index(0);
            return;
        }

        let current = self.selected_index() as isize;
        let next = (current + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
        self.set_selected_index(next);
    }

    pub fn switch_tab(&mut self, next: bool) {
        self.tab = if next {
            self.tab.next()
        } else {
            self.tab.previous()
        };
        let max_index = self.items().len().saturating_sub(1);
        if self.selected_index() > max_index {
            self.set_selected_index(max_index);
        }
    }

    pub fn activate_selected(&mut self) -> Result<ActivateResult> {
        let Some(item) = self.items().get(self.selected_index()).cloned() else {
            return Ok(ActivateResult::Changed);
        };

        match item.value {
            SettingsValue::Bool(value) => {
                self.set_bool(item.field, !value);
                Ok(ActivateResult::Changed)
            }
            SettingsValue::Cycle { current, options } => {
                let next = next_cycle_value(&current, options);
                self.apply_cycle(item.field, &next)?;
                Ok(ActivateResult::Changed)
            }
            SettingsValue::Number(value) => {
                self.editing = true;
                self.edit_buffer = value.to_string();
                Ok(ActivateResult::EditingStarted)
            }
            SettingsValue::Text(value)
            | SettingsValue::Key(value)
            | SettingsValue::Color(value) => {
                self.editing = true;
                self.edit_buffer = value;
                Ok(ActivateResult::EditingStarted)
            }
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    pub fn confirm_edit(&mut self) -> Result<()> {
        let Some(item) = self.items().get(self.selected_index()).cloned() else {
            self.cancel_edit();
            return Ok(());
        };

        match item.value {
            SettingsValue::Number(_) => {
                let value = self
                    .edit_buffer
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| anyhow!("{} expects a whole number.", item.name))?;
                self.apply_number(item.field, value)?;
            }
            SettingsValue::Text(_) => {
                self.apply_text(item.field, self.edit_buffer.clone())?;
            }
            SettingsValue::Color(_) => {
                parse_color(&self.edit_buffer)?;
                self.apply_text(item.field, self.edit_buffer.clone())?;
                self.draft.mark_theme_custom();
            }
            SettingsValue::Key(_) => {
                let normalized = normalize_keybinding(&self.edit_buffer)?;
                self.apply_text(item.field, normalized)?;
            }
            SettingsValue::Bool(_) | SettingsValue::Cycle { .. } => {}
        }

        self.cancel_edit();
        Ok(())
    }

    pub fn capture_keybinding(&mut self, key: KeyEvent) -> Result<String> {
        let Some(binding) = event_to_keybinding(key) else {
            return Err(anyhow!("That key cannot be assigned here."));
        };

        let normalized = normalize_keybinding(&binding)?;
        let Some(item) = self.items().get(self.selected_index()).cloned() else {
            return Err(anyhow!("No keybinding is selected."));
        };

        let SettingField::Keybinding(action) = item.field else {
            return Err(anyhow!("The selected setting is not a keybinding."));
        };

        for candidate in KeyAction::ALL {
            if candidate == action {
                continue;
            }

            if normalize_keybinding(self.draft.keybinding(candidate))? == normalized {
                return Err(anyhow!(
                    "{} is already assigned to {}.",
                    normalized,
                    candidate.label()
                ));
            }
        }

        self.draft.set_keybinding(action, normalized.clone());
        self.cancel_edit();
        Ok(normalized)
    }

    pub fn has_unsaved_changes(&self, settings: &Settings) -> bool {
        self.draft != *settings
    }

    fn set_bool(&mut self, field: SettingField, value: bool) {
        match field {
            SettingField::Behavior(BehaviorField::TextEmphasis) => self.draft.text_emphasis = value,
            SettingField::Behavior(BehaviorField::LoadingIndicator) => {
                self.draft.show_loading_indicator = value
            }
            SettingField::Behavior(BehaviorField::WideSearchBar) => {
                self.draft.wide_search_bar = value
            }
            SettingField::Behavior(BehaviorField::SetWindowTitle) => {
                self.draft.set_window_title = value
            }
            SettingField::Behavior(BehaviorField::StopAfterCurrentTrack) => {
                self.draft.stop_after_current_track = value
            }
            SettingField::Behavior(BehaviorField::DrawCoverArt) => {
                self.draft.draw_cover_art = value
            }
            SettingField::Behavior(BehaviorField::ForceDrawCoverArt) => {
                self.draft.force_draw_cover_art = value
            }
            _ => {}
        }
    }

    fn apply_cycle(&mut self, field: SettingField, value: &str) -> Result<()> {
        match field {
            SettingField::Behavior(BehaviorField::StartupBehavior) => {
                self.draft.startup_behavior = StartupBehavior::from_label(value);
            }
            SettingField::ThemePreset => {
                if !self.draft.apply_theme_preset(value) {
                    return Err(anyhow!("Unknown theme preset '{}'.", value));
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn apply_number(&mut self, field: SettingField, value: i64) -> Result<()> {
        match field {
            SettingField::Behavior(BehaviorField::SeekDuration) => {
                self.draft.seek_duration_ms = value.max(0) as u64;
            }
            SettingField::Behavior(BehaviorField::VolumeIncrement) => {
                if !(0..=100).contains(&value) {
                    return Err(anyhow!("Volume Increment must stay between 0 and 100."));
                }
                self.draft.volume_increment = value as u8;
            }
            SettingField::Behavior(BehaviorField::TickRate) => {
                if value <= 0 {
                    return Err(anyhow!("Tick Rate (ms) must be at least 1."));
                }
                self.draft.tick_rate_ms = value as u64;
            }
            _ => {}
        }

        Ok(())
    }

    fn apply_text(&mut self, field: SettingField, value: String) -> Result<()> {
        match field {
            SettingField::Behavior(BehaviorField::LikedIcon) => self.draft.liked_icon = value,
            SettingField::Behavior(BehaviorField::ShuffleIcon) => self.draft.shuffle_icon = value,
            SettingField::Behavior(BehaviorField::PlayingIcon) => self.draft.playing_icon = value,
            SettingField::Behavior(BehaviorField::PausedIcon) => self.draft.paused_icon = value,
            SettingField::Keybinding(action) => self.draft.set_keybinding(action, value),
            SettingField::ThemeColor(ThemeColorField::Active) => self.draft.active_color = value,
            SettingField::ThemeColor(ThemeColorField::Banner) => self.draft.banner_color = value,
            SettingField::ThemeColor(ThemeColorField::Hint) => self.draft.hint_color = value,
            SettingField::ThemeColor(ThemeColorField::Hovered) => self.draft.hovered_color = value,
            SettingField::ThemeColor(ThemeColorField::Selected) => {
                self.draft.selected_color = value
            }
            SettingField::ThemeColor(ThemeColorField::Inactive) => {
                self.draft.inactive_color = value
            }
            SettingField::ThemeColor(ThemeColorField::Text) => self.draft.text_color = value,
            SettingField::ThemeColor(ThemeColorField::ErrorText) => {
                self.draft.error_text_color = value
            }
            SettingField::ThemeColor(ThemeColorField::PlaybarBackground) => {
                self.draft.playbar_background = value
            }
            SettingField::ThemeColor(ThemeColorField::PlaybarProgress) => {
                self.draft.playbar_progress = value
            }
            SettingField::ThemeColor(ThemeColorField::LyricsHighlight) => {
                self.draft.lyrics_highlight = value
            }
            _ => return Err(anyhow!("The selected setting cannot be edited as text.")),
        }

        Ok(())
    }
}

fn behavior_items(settings: &Settings) -> Vec<SettingsItem> {
    vec![
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::SeekDuration),
            name: "Seek Duration (ms)",
            value: SettingsValue::Number(settings.seek_duration_ms as i64),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::VolumeIncrement),
            name: "Volume Increment",
            value: SettingsValue::Number(settings.volume_increment as i64),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::TickRate),
            name: "Tick Rate (ms)",
            value: SettingsValue::Number(settings.tick_rate_ms as i64),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::TextEmphasis),
            name: "Text Emphasis",
            value: SettingsValue::Bool(settings.text_emphasis),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::LoadingIndicator),
            name: "Loading Indicator",
            value: SettingsValue::Bool(settings.show_loading_indicator),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::WideSearchBar),
            name: "Wide Search Bar",
            value: SettingsValue::Bool(settings.wide_search_bar),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::SetWindowTitle),
            name: "Set Window Title",
            value: SettingsValue::Bool(settings.set_window_title),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::StopAfterCurrentTrack),
            name: "Stop After Current Track",
            value: SettingsValue::Bool(settings.stop_after_current_track),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::StartupBehavior),
            name: "Startup Behavior",
            value: SettingsValue::Cycle {
                current: settings.startup_behavior.label().to_string(),
                options: &StartupBehavior::OPTIONS,
            },
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::LikedIcon),
            name: "Liked Icon",
            value: SettingsValue::Text(settings.liked_icon.clone()),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::ShuffleIcon),
            name: "Shuffle Icon",
            value: SettingsValue::Text(settings.shuffle_icon.clone()),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::PlayingIcon),
            name: "Playing Icon",
            value: SettingsValue::Text(settings.playing_icon.clone()),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::PausedIcon),
            name: "Paused Icon",
            value: SettingsValue::Text(settings.paused_icon.clone()),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::DrawCoverArt),
            name: "Draw Cover Art",
            value: SettingsValue::Bool(settings.draw_cover_art),
        },
        SettingsItem {
            field: SettingField::Behavior(BehaviorField::ForceDrawCoverArt),
            name: "Force Draw Cover Art",
            value: SettingsValue::Bool(settings.force_draw_cover_art),
        },
    ]
}

fn keybinding_items(settings: &Settings) -> Vec<SettingsItem> {
    KeyAction::ALL
        .into_iter()
        .map(|action| SettingsItem {
            field: SettingField::Keybinding(action),
            name: action.label(),
            value: SettingsValue::Key(settings.keybinding(action).to_string()),
        })
        .collect()
}

fn theme_items(settings: &Settings) -> Vec<SettingsItem> {
    vec![
        SettingsItem {
            field: SettingField::ThemePreset,
            name: "Theme Preset",
            value: SettingsValue::Cycle {
                current: settings.theme.clone(),
                options: theme_preset_names(),
            },
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Active),
            name: "Active Color",
            value: SettingsValue::Color(settings.active_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Banner),
            name: "Banner Color",
            value: SettingsValue::Color(settings.banner_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Hint),
            name: "Hint Color",
            value: SettingsValue::Color(settings.hint_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Hovered),
            name: "Hovered Color",
            value: SettingsValue::Color(settings.hovered_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Selected),
            name: "Selected Color",
            value: SettingsValue::Color(settings.selected_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Inactive),
            name: "Inactive Color",
            value: SettingsValue::Color(settings.inactive_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::Text),
            name: "Text Color",
            value: SettingsValue::Color(settings.text_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::ErrorText),
            name: "Error Text Color",
            value: SettingsValue::Color(settings.error_text_color.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::PlaybarBackground),
            name: "Playbar Background",
            value: SettingsValue::Color(settings.playbar_background.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::PlaybarProgress),
            name: "Playbar Progress",
            value: SettingsValue::Color(settings.playbar_progress.clone()),
        },
        SettingsItem {
            field: SettingField::ThemeColor(ThemeColorField::LyricsHighlight),
            name: "Lyrics Highlight",
            value: SettingsValue::Color(settings.lyrics_highlight.clone()),
        },
    ]
}

fn next_cycle_value(current: &str, options: &'static [&'static str]) -> String {
    match options.iter().position(|option| *option == current) {
        Some(index) => options[(index + 1) % options.len()].to_string(),
        None => options[0].to_string(),
    }
}
