use std::fs;

use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::config::paths::AppPaths;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupBehavior {
    Continue,
    Play,
    Pause,
}

impl StartupBehavior {
    pub const OPTIONS: [&'static str; 3] = ["Continue", "Play", "Pause"];

    pub fn label(self) -> &'static str {
        match self {
            Self::Continue => "Continue",
            Self::Play => "Play",
            Self::Pause => "Pause",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label {
            "Play" => Self::Play,
            "Pause" => Self::Pause,
            _ => Self::Continue,
        }
    }
}

impl Default for StartupBehavior {
    fn default() -> Self {
        Self::Continue
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KeyAction {
    Back,
    NextPage,
    PreviousPage,
    TogglePlayback,
    SeekBackwards,
    SeekForwards,
    NextTrack,
    PreviousTrack,
    ForcePreviousTrack,
    Shuffle,
    Repeat,
    Search,
    Help,
    OpenSettings,
    SaveSettings,
    DecreaseVolume,
    IncreaseVolume,
    AddToQueue,
    ShowQueue,
    CopySongUrl,
}

impl KeyAction {
    pub const ALL: [Self; 20] = [
        Self::Back,
        Self::NextPage,
        Self::PreviousPage,
        Self::TogglePlayback,
        Self::SeekBackwards,
        Self::SeekForwards,
        Self::NextTrack,
        Self::PreviousTrack,
        Self::ForcePreviousTrack,
        Self::Shuffle,
        Self::Repeat,
        Self::Search,
        Self::Help,
        Self::OpenSettings,
        Self::SaveSettings,
        Self::DecreaseVolume,
        Self::IncreaseVolume,
        Self::AddToQueue,
        Self::ShowQueue,
        Self::CopySongUrl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Back => "Back",
            Self::NextPage => "Next Page",
            Self::PreviousPage => "Previous Page",
            Self::TogglePlayback => "Toggle Playback",
            Self::SeekBackwards => "Seek Backwards",
            Self::SeekForwards => "Seek Forwards",
            Self::NextTrack => "Next Track",
            Self::PreviousTrack => "Previous Track",
            Self::ForcePreviousTrack => "Force Previous Track",
            Self::Shuffle => "Shuffle",
            Self::Repeat => "Repeat",
            Self::Search => "Search",
            Self::Help => "Help",
            Self::OpenSettings => "Open Settings",
            Self::SaveSettings => "Save Settings",
            Self::DecreaseVolume => "Decrease Volume",
            Self::IncreaseVolume => "Increase Volume",
            Self::AddToQueue => "Add to Queue",
            Self::ShowQueue => "Show Queue",
            Self::CopySongUrl => "Copy Song URL",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub show_help_on_startup: bool,
    pub seek_duration_ms: u64,
    pub volume_increment: u8,
    pub tick_rate_ms: u64,
    pub text_emphasis: bool,
    pub show_loading_indicator: bool,
    pub wide_search_bar: bool,
    pub set_window_title: bool,
    pub stop_after_current_track: bool,
    pub startup_behavior: StartupBehavior,
    pub liked_icon: String,
    pub shuffle_icon: String,
    pub playing_icon: String,
    pub paused_icon: String,
    pub draw_cover_art: bool,
    pub force_draw_cover_art: bool,
    pub key_back: String,
    pub key_next_page: String,
    pub key_previous_page: String,
    pub key_toggle_playback: String,
    pub key_seek_backwards: String,
    pub key_seek_forwards: String,
    pub key_next_track: String,
    pub key_previous_track: String,
    pub key_force_previous_track: String,
    pub key_shuffle: String,
    pub key_repeat: String,
    pub key_search: String,
    pub key_help: String,
    pub key_open_settings: String,
    pub key_save_settings: String,
    pub key_decrease_volume: String,
    pub key_increase_volume: String,
    pub key_add_to_queue: String,
    pub key_show_queue: String,
    pub key_copy_song_url: String,
    pub active_color: String,
    pub banner_color: String,
    pub hint_color: String,
    pub hovered_color: String,
    pub selected_color: String,
    pub inactive_color: String,
    pub text_color: String,
    pub error_text_color: String,
    pub playbar_background: String,
    pub playbar_progress: String,
    pub lyrics_highlight: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "SoundCloud".to_string(),
            show_help_on_startup: true,
            seek_duration_ms: 5000,
            volume_increment: 10,
            tick_rate_ms: 250,
            text_emphasis: true,
            show_loading_indicator: true,
            wide_search_bar: false,
            set_window_title: true,
            stop_after_current_track: false,
            startup_behavior: StartupBehavior::Continue,
            liked_icon: "♥".to_string(),
            shuffle_icon: "🔀".to_string(),
            playing_icon: "▶".to_string(),
            paused_icon: "⏸".to_string(),
            draw_cover_art: true,
            force_draw_cover_art: false,
            key_back: "q".to_string(),
            key_next_page: "ctrl-d".to_string(),
            key_previous_page: "ctrl-u".to_string(),
            key_toggle_playback: "space".to_string(),
            key_seek_backwards: "<".to_string(),
            key_seek_forwards: ">".to_string(),
            key_next_track: "n".to_string(),
            key_previous_track: "p".to_string(),
            key_force_previous_track: "P".to_string(),
            key_shuffle: "ctrl-s".to_string(),
            key_repeat: "ctrl-r".to_string(),
            key_search: "/".to_string(),
            key_help: "?".to_string(),
            key_open_settings: default_open_settings_binding().to_string(),
            key_save_settings: "alt-s".to_string(),
            key_decrease_volume: "-".to_string(),
            key_increase_volume: "+".to_string(),
            key_add_to_queue: "z".to_string(),
            key_show_queue: "Q".to_string(),
            key_copy_song_url: "c".to_string(),
            active_color: "255, 95, 31".to_string(),
            banner_color: "255, 142, 42".to_string(),
            hint_color: "255, 196, 103".to_string(),
            hovered_color: "255, 142, 42".to_string(),
            selected_color: "255, 95, 31".to_string(),
            inactive_color: "136, 118, 108".to_string(),
            text_color: "Reset".to_string(),
            error_text_color: "255, 100, 100".to_string(),
            playbar_background: "Reset".to_string(),
            playbar_progress: "255, 95, 31".to_string(),
            lyrics_highlight: "255, 95, 31".to_string(),
        }
    }
}

impl Settings {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        if !paths.settings_file.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&paths.settings_file)?;
        let mut settings = toml::from_str::<Self>(&raw)?;
        settings.normalize();
        Ok(settings)
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let mut normalized = self.clone();
        normalized.normalize();
        normalized.validate()?;
        let contents = toml::to_string_pretty(&normalized)?;
        fs::write(&paths.settings_file, contents)?;
        Ok(())
    }

    pub fn normalize(&mut self) {
        for action in KeyAction::ALL {
            let value = self.keybinding(action).to_string();
            if let Ok(normalized) = normalize_keybinding(&value) {
                self.set_keybinding(action, normalized);
            }
        }

        if !theme_preset_names().iter().any(|name| *name == self.theme) {
            self.theme = "SoundCloud".to_string();
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.tick_rate_ms == 0 {
            return Err(anyhow!("Tick Rate (ms) must be at least 1."));
        }

        if self.tick_rate_ms > 5000 {
            return Err(anyhow!("Tick Rate (ms) must be 5000 or less."));
        }

        let mut seen: Vec<(KeyAction, String)> = Vec::new();
        for action in KeyAction::ALL {
            let normalized = normalize_keybinding(self.keybinding(action))?;
            if is_reserved_keybinding(&normalized) {
                return Err(anyhow!(
                    "{} cannot use the reserved binding '{}'.",
                    action.label(),
                    normalized
                ));
            }

            if let Some((other_action, _)) = seen.iter().find(|(_, value)| *value == normalized) {
                return Err(anyhow!(
                    "{} conflicts with {} on binding '{}'.",
                    action.label(),
                    other_action.label(),
                    normalized
                ));
            }

            seen.push((action, normalized));
        }

        for color in [
            self.active_color.as_str(),
            self.banner_color.as_str(),
            self.hint_color.as_str(),
            self.hovered_color.as_str(),
            self.selected_color.as_str(),
            self.inactive_color.as_str(),
            self.text_color.as_str(),
            self.error_text_color.as_str(),
            self.playbar_background.as_str(),
            self.playbar_progress.as_str(),
            self.lyrics_highlight.as_str(),
        ] {
            parse_color(color)?;
        }

        Ok(())
    }

    pub fn keybinding(&self, action: KeyAction) -> &str {
        match action {
            KeyAction::Back => &self.key_back,
            KeyAction::NextPage => &self.key_next_page,
            KeyAction::PreviousPage => &self.key_previous_page,
            KeyAction::TogglePlayback => &self.key_toggle_playback,
            KeyAction::SeekBackwards => &self.key_seek_backwards,
            KeyAction::SeekForwards => &self.key_seek_forwards,
            KeyAction::NextTrack => &self.key_next_track,
            KeyAction::PreviousTrack => &self.key_previous_track,
            KeyAction::ForcePreviousTrack => &self.key_force_previous_track,
            KeyAction::Shuffle => &self.key_shuffle,
            KeyAction::Repeat => &self.key_repeat,
            KeyAction::Search => &self.key_search,
            KeyAction::Help => &self.key_help,
            KeyAction::OpenSettings => &self.key_open_settings,
            KeyAction::SaveSettings => &self.key_save_settings,
            KeyAction::DecreaseVolume => &self.key_decrease_volume,
            KeyAction::IncreaseVolume => &self.key_increase_volume,
            KeyAction::AddToQueue => &self.key_add_to_queue,
            KeyAction::ShowQueue => &self.key_show_queue,
            KeyAction::CopySongUrl => &self.key_copy_song_url,
        }
    }

    pub fn set_keybinding(&mut self, action: KeyAction, value: impl Into<String>) {
        let value = value.into();
        match action {
            KeyAction::Back => self.key_back = value,
            KeyAction::NextPage => self.key_next_page = value,
            KeyAction::PreviousPage => self.key_previous_page = value,
            KeyAction::TogglePlayback => self.key_toggle_playback = value,
            KeyAction::SeekBackwards => self.key_seek_backwards = value,
            KeyAction::SeekForwards => self.key_seek_forwards = value,
            KeyAction::NextTrack => self.key_next_track = value,
            KeyAction::PreviousTrack => self.key_previous_track = value,
            KeyAction::ForcePreviousTrack => self.key_force_previous_track = value,
            KeyAction::Shuffle => self.key_shuffle = value,
            KeyAction::Repeat => self.key_repeat = value,
            KeyAction::Search => self.key_search = value,
            KeyAction::Help => self.key_help = value,
            KeyAction::OpenSettings => self.key_open_settings = value,
            KeyAction::SaveSettings => self.key_save_settings = value,
            KeyAction::DecreaseVolume => self.key_decrease_volume = value,
            KeyAction::IncreaseVolume => self.key_increase_volume = value,
            KeyAction::AddToQueue => self.key_add_to_queue = value,
            KeyAction::ShowQueue => self.key_show_queue = value,
            KeyAction::CopySongUrl => self.key_copy_song_url = value,
        }
    }

    pub fn key_matches(&self, action: KeyAction, key: KeyEvent) -> bool {
        event_to_keybinding(key)
            .and_then(|event| normalize_keybinding(&event).ok())
            .map(|event| event == normalize_keybinding(self.keybinding(action)).unwrap_or_default())
            .unwrap_or(false)
    }

    pub fn apply_theme_preset(&mut self, preset: &str) -> bool {
        let Some(palette) = theme_palette(preset) else {
            return false;
        };

        self.theme = palette.name.to_string();
        self.active_color = palette.active.to_string();
        self.banner_color = palette.banner.to_string();
        self.hint_color = palette.hint.to_string();
        self.hovered_color = palette.hovered.to_string();
        self.selected_color = palette.selected.to_string();
        self.inactive_color = palette.inactive.to_string();
        self.text_color = palette.text.to_string();
        self.error_text_color = palette.error_text.to_string();
        self.playbar_background = palette.playbar_background.to_string();
        self.playbar_progress = palette.playbar_progress.to_string();
        self.lyrics_highlight = palette.lyrics_highlight.to_string();
        true
    }

    pub fn mark_theme_custom(&mut self) {
        self.theme = "Custom".to_string();
    }
}

pub fn ensure_default_file(paths: &AppPaths) -> Result<()> {
    if paths.settings_file.exists() {
        return Ok(());
    }

    Settings::default().save(paths)?;

    Ok(())
}

pub fn theme_preset_names() -> &'static [&'static str] {
    &[
        "SoundCloud",
        "Cyan",
        "Pookie Pink",
        "Spotify",
        "Vesper",
        "Dracula",
        "Nord",
        "Solarized Dark",
        "Monokai",
        "Gruvbox",
        "Gruvbox Light",
        "Catppuccin Mocha",
    ]
}

pub fn event_to_keybinding(key: KeyEvent) -> Option<String> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return None;
    }

    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    if control && alt {
        return None;
    }

    let binding = match key.code {
        KeyCode::Char(' ') if !control && !alt => "space".to_string(),
        KeyCode::Char(ch) if control => format!("ctrl-{}", ch.to_ascii_lowercase()),
        KeyCode::Char(ch) if alt => format!("alt-{ch}"),
        KeyCode::Char(ch) => ch.to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Delete => "del".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::Insert => "ins".to_string(),
        KeyCode::F(number) => format!("f{number}"),
        _ => return None,
    };

    Some(binding)
}

pub fn normalize_keybinding(binding: &str) -> Result<String> {
    let binding = binding.trim();
    if binding.is_empty() {
        return Err(anyhow!("Keybinding cannot be empty."));
    }

    let lower = binding.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("ctrl-") {
        return normalize_modifier_binding("ctrl", rest);
    }

    if let Some(rest) = lower.strip_prefix("alt-") {
        return normalize_modifier_binding("alt", rest);
    }

    let normalized = match lower.as_str() {
        "space" | "enter" | "esc" | "escape" | "backspace" | "del" | "delete" | "left"
        | "right" | "up" | "down" | "pageup" | "pagedown" | "home" | "end" | "tab" | "ins"
        | "insert" => match lower.as_str() {
            "escape" => "esc".to_string(),
            "delete" => "del".to_string(),
            "insert" => "ins".to_string(),
            other => other.to_string(),
        },
        _ if lower.starts_with('f') && lower[1..].parse::<u8>().is_ok() => lower,
        _ if binding.chars().count() == 1 => binding.to_string(),
        _ => {
            return Err(anyhow!(
                "Unsupported keybinding '{}'. Use a single key, ctrl-<key>, alt-<key>, or a named key like enter.",
                binding
            ));
        }
    };

    Ok(normalized)
}

pub fn parse_color(value: &str) -> Result<Color> {
    let value = value.trim();
    let color = match value {
        "Reset" => Color::Reset,
        "Black" => Color::Black,
        "Red" => Color::Red,
        "Green" => Color::Green,
        "Yellow" => Color::Yellow,
        "Blue" => Color::Blue,
        "Magenta" => Color::Magenta,
        "Cyan" => Color::Cyan,
        "Gray" => Color::Gray,
        "DarkGray" => Color::DarkGray,
        "LightRed" => Color::LightRed,
        "LightGreen" => Color::LightGreen,
        "LightYellow" => Color::LightYellow,
        "LightBlue" => Color::LightBlue,
        "LightMagenta" => Color::LightMagenta,
        "LightCyan" => Color::LightCyan,
        "White" => Color::White,
        _ => {
            let mut parts = value.split(',').map(str::trim);
            let Some(r) = parts.next() else {
                return Err(anyhow!("Invalid color '{}'.", value));
            };
            let Some(g) = parts.next() else {
                return Err(anyhow!("Invalid color '{}'.", value));
            };
            let Some(b) = parts.next() else {
                return Err(anyhow!("Invalid color '{}'.", value));
            };
            if parts.next().is_some() {
                return Err(anyhow!("Invalid color '{}'.", value));
            }
            Color::Rgb(r.parse()?, g.parse()?, b.parse()?)
        }
    };

    Ok(color)
}

fn normalize_modifier_binding(prefix: &str, rest: &str) -> Result<String> {
    if rest.chars().count() != 1 {
        return Err(anyhow!(
            "{} bindings must target a single key, got '{}'.",
            prefix,
            rest
        ));
    }

    let ch = rest.chars().next().expect("single char binding");
    let normalized = if ch.is_ascii_alphabetic() {
        ch.to_ascii_lowercase().to_string()
    } else {
        ch.to_string()
    };
    Ok(format!("{prefix}-{normalized}"))
}

fn is_reserved_keybinding(binding: &str) -> bool {
    matches!(
        binding,
        "h" | "j"
            | "k"
            | "l"
            | "H"
            | "M"
            | "L"
            | "up"
            | "down"
            | "left"
            | "right"
            | "backspace"
            | "enter"
    )
}

fn default_open_settings_binding() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "ctrl-,"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "alt-,"
    }
}

#[derive(Clone, Copy)]
struct ThemePalette {
    name: &'static str,
    active: &'static str,
    banner: &'static str,
    hint: &'static str,
    hovered: &'static str,
    selected: &'static str,
    inactive: &'static str,
    text: &'static str,
    error_text: &'static str,
    playbar_background: &'static str,
    playbar_progress: &'static str,
    lyrics_highlight: &'static str,
}

fn theme_palette(name: &str) -> Option<ThemePalette> {
    THEME_PALETTES
        .iter()
        .copied()
        .find(|palette| palette.name == name)
}

const THEME_PALETTES: [ThemePalette; 12] = [
    ThemePalette {
        name: "SoundCloud",
        active: "255, 95, 31",
        banner: "255, 142, 42",
        hint: "255, 196, 103",
        hovered: "255, 142, 42",
        selected: "255, 95, 31",
        inactive: "136, 118, 108",
        text: "Reset",
        error_text: "255, 100, 100",
        playbar_background: "Reset",
        playbar_progress: "255, 95, 31",
        lyrics_highlight: "255, 95, 31",
    },
    ThemePalette {
        name: "Cyan",
        active: "0, 180, 180",
        banner: "0, 200, 200",
        hint: "200, 200, 0",
        hovered: "180, 0, 180",
        selected: "0, 200, 200",
        inactive: "128, 128, 128",
        text: "Reset",
        error_text: "255, 100, 100",
        playbar_background: "Reset",
        playbar_progress: "0, 200, 200",
        lyrics_highlight: "0, 200, 200",
    },
    ThemePalette {
        name: "Pookie Pink",
        active: "150, 25, 92",
        banner: "255, 145, 205",
        hint: "255, 235, 245",
        hovered: "220, 85, 155",
        selected: "125, 20, 80",
        inactive: "255, 195, 225",
        text: "255, 255, 255",
        error_text: "255, 215, 235",
        playbar_background: "245, 115, 180",
        playbar_progress: "255, 255, 255",
        lyrics_highlight: "255, 230, 245",
    },
    ThemePalette {
        name: "Spotify",
        active: "29, 185, 84",
        banner: "29, 185, 84",
        hint: "179, 179, 179",
        hovered: "29, 185, 84",
        selected: "29, 185, 84",
        inactive: "83, 83, 83",
        text: "255, 255, 255",
        error_text: "230, 76, 76",
        playbar_background: "Reset",
        playbar_progress: "29, 185, 84",
        lyrics_highlight: "29, 185, 84",
    },
    ThemePalette {
        name: "Vesper",
        active: "255, 199, 153",
        banner: "255, 199, 153",
        hint: "255, 199, 153",
        hovered: "255, 207, 168",
        selected: "255, 199, 153",
        inactive: "190, 190, 190",
        text: "255, 255, 255",
        error_text: "255, 128, 128",
        playbar_background: "22, 22, 22",
        playbar_progress: "153, 255, 228",
        lyrics_highlight: "153, 255, 228",
    },
    ThemePalette {
        name: "Dracula",
        active: "80, 250, 123",
        banner: "255, 121, 198",
        hint: "241, 250, 140",
        hovered: "189, 147, 249",
        selected: "139, 233, 253",
        inactive: "98, 114, 164",
        text: "248, 248, 242",
        error_text: "255, 85, 85",
        playbar_background: "Reset",
        playbar_progress: "80, 250, 123",
        lyrics_highlight: "255, 121, 198",
    },
    ThemePalette {
        name: "Nord",
        active: "163, 190, 140",
        banner: "136, 192, 208",
        hint: "235, 203, 139",
        hovered: "180, 142, 173",
        selected: "129, 161, 193",
        inactive: "76, 86, 106",
        text: "236, 239, 244",
        error_text: "191, 97, 106",
        playbar_background: "Reset",
        playbar_progress: "136, 192, 208",
        lyrics_highlight: "136, 192, 208",
    },
    ThemePalette {
        name: "Solarized Dark",
        active: "133, 153, 0",
        banner: "38, 139, 210",
        hint: "181, 137, 0",
        hovered: "211, 54, 130",
        selected: "42, 161, 152",
        inactive: "88, 110, 117",
        text: "147, 161, 161",
        error_text: "220, 50, 47",
        playbar_background: "Reset",
        playbar_progress: "42, 161, 152",
        lyrics_highlight: "38, 139, 210",
    },
    ThemePalette {
        name: "Monokai",
        active: "166, 226, 46",
        banner: "249, 38, 114",
        hint: "230, 219, 116",
        hovered: "174, 129, 255",
        selected: "102, 217, 239",
        inactive: "117, 113, 94",
        text: "248, 248, 242",
        error_text: "249, 38, 114",
        playbar_background: "Reset",
        playbar_progress: "166, 226, 46",
        lyrics_highlight: "249, 38, 114",
    },
    ThemePalette {
        name: "Gruvbox",
        active: "184, 187, 38",
        banner: "254, 128, 25",
        hint: "250, 189, 47",
        hovered: "211, 134, 155",
        selected: "131, 165, 152",
        inactive: "146, 131, 116",
        text: "235, 219, 178",
        error_text: "251, 73, 52",
        playbar_background: "Reset",
        playbar_progress: "184, 187, 38",
        lyrics_highlight: "254, 128, 25",
    },
    ThemePalette {
        name: "Gruvbox Light",
        active: "121, 116, 14",
        banner: "175, 58, 3",
        hint: "181, 118, 20",
        hovered: "143, 63, 113",
        selected: "66, 123, 88",
        inactive: "146, 131, 116",
        text: "60, 56, 54",
        error_text: "157, 0, 6",
        playbar_background: "251, 241, 199",
        playbar_progress: "121, 116, 14",
        lyrics_highlight: "175, 58, 3",
    },
    ThemePalette {
        name: "Catppuccin Mocha",
        active: "180, 190, 254",
        banner: "180, 190, 254",
        hint: "250, 179, 135",
        hovered: "137, 180, 250",
        selected: "180, 190, 254",
        inactive: "108, 112, 134",
        text: "205, 214, 244",
        error_text: "243, 139, 168",
        playbar_background: "Reset",
        playbar_progress: "180, 190, 254",
        lyrics_highlight: "180, 190, 254",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_char_keybindings_stay_case_sensitive() {
        assert_eq!(normalize_keybinding("Q").expect("normalized"), "Q");
        assert_eq!(normalize_keybinding("q").expect("normalized"), "q");
    }

    #[test]
    fn modifier_keybindings_normalize_letters() {
        assert_eq!(
            normalize_keybinding("ctrl-D").expect("normalized"),
            "ctrl-d"
        );
        assert_eq!(normalize_keybinding("alt-S").expect("normalized"), "alt-s");
    }

    #[test]
    fn parse_rgb_colors() {
        assert_eq!(
            parse_color("255, 95, 31").expect("color"),
            Color::Rgb(255, 95, 31)
        );
    }
}
