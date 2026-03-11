use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};

use crate::{
    app::{AppState, Route, SettingsTab},
    ui::widgets::pane_inner,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MainLayout {
    pub frame: Rect,
    pub app: Rect,
    pub header: Option<Rect>,
    pub body: Rect,
    pub playbar: Rect,
    pub search: Rect,
    pub help: Rect,
    pub settings: Rect,
    pub library: Rect,
    pub playlists: Rect,
    pub content: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct OverlayLayout {
    pub overlay: Rect,
    pub body: Rect,
    pub footer: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SettingsLayout {
    pub overlay: Rect,
    pub tabs: Rect,
    pub list: Rect,
    pub footer: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ContentLayout {
    pub inner: Rect,
    pub summary: Option<Rect>,
    pub body: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct AddToPlaylistLayout {
    pub overlay: Rect,
    pub prompt: Rect,
    pub list: Rect,
    pub footer: Rect,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ErrorLayout {
    pub overlay: Rect,
    pub body: Rect,
}

pub const SETTINGS_TAB_LEFT_PADDING: &str = " ";
pub const SETTINGS_TAB_RIGHT_PADDING: &str = " ";
pub const SETTINGS_TAB_DIVIDER: &str = "|";
const HEADER_COMPONENT_SPACING: u16 = 0;

fn wide_header_constraints() -> [Constraint; 3] {
    [
        Constraint::Min(1),
        Constraint::Length(14),
        Constraint::Length(18),
    ]
}

fn compact_header_constraints() -> [Constraint; 3] {
    [
        Constraint::Ratio(11, 20),
        Constraint::Ratio(3, 20),
        Constraint::Ratio(6, 20),
    ]
}

pub fn viewport_area(app: &AppState) -> Option<Rect> {
    if app.viewport.width == 0 || app.viewport.height == 0 {
        return None;
    }

    Some(Rect::new(0, 0, app.viewport.width, app.viewport.height))
}

pub fn main_layout_from_viewport(app: &AppState) -> Option<MainLayout> {
    viewport_area(app).map(|area| main_layout(area, app))
}

pub fn main_layout(frame: Rect, app: &AppState) -> MainLayout {
    let app_area = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(frame)[0];

    let wide_search = app.settings().wide_search_bar;
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints(if wide_search {
            vec![
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(app.layout.playbar_height.max(4)),
            ]
        } else {
            vec![
                Constraint::Min(1),
                Constraint::Length(app.layout.playbar_height.max(4)),
            ]
        })
        .split(app_area);

    let (header, body, playbar) = if wide_search {
        (Some(vertical[0]), vertical[1], vertical[2])
    } else {
        (None, vertical[0], vertical[1])
    };

    let sidebar_width = app.layout.sidebar_width_percent.clamp(14, 40);
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(1)
        .constraints([
            Constraint::Percentage(sidebar_width),
            Constraint::Percentage(100 - sidebar_width),
        ])
        .split(body);

    let library_height = app.layout.library_height.max(4);
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints(if wide_search {
            vec![Constraint::Length(library_height), Constraint::Min(1)]
        } else {
            vec![
                Constraint::Length(3),
                Constraint::Length(library_height),
                Constraint::Min(1),
            ]
        })
        .split(middle[0]);

    let (search, help, settings, library, playlists) = if let Some(header_area) = header {
        let header_sections = Layout::default()
            .direction(Direction::Horizontal)
            .spacing(HEADER_COMPONENT_SPACING)
            .constraints(wide_header_constraints())
            .split(header_area);
        (
            header_sections[0],
            header_sections[1],
            header_sections[2],
            sidebar[0],
            sidebar[1],
        )
    } else {
        let sidebar_header = Layout::default()
            .direction(Direction::Horizontal)
            .spacing(HEADER_COMPONENT_SPACING)
            .constraints(compact_header_constraints())
            .split(sidebar[0]);
        (
            sidebar_header[0],
            sidebar_header[1],
            sidebar_header[2],
            sidebar[1],
            sidebar[2],
        )
    };

    MainLayout {
        frame,
        app: app_area,
        header,
        body,
        playbar,
        search,
        help,
        settings,
        library,
        playlists,
        content: middle[1],
    }
}

pub fn content_layout(area: Rect, app: &AppState) -> ContentLayout {
    let inner = pane_inner(area);
    let summary_lines = content_summary_line_count(app);

    if summary_lines == 0 {
        return ContentLayout {
            inner,
            summary: None,
            body: inner,
        };
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(summary_lines), Constraint::Min(1)])
        .split(inner);

    ContentLayout {
        inner,
        summary: Some(chunks[0]),
        body: chunks[1],
    }
}

pub fn help_layout(area: Rect) -> OverlayLayout {
    let overlay = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(area)[0];
    let inner = pane_inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    OverlayLayout {
        overlay,
        body: sections[0],
        footer: sections[1],
    }
}

pub fn queue_layout(area: Rect) -> OverlayLayout {
    let overlay = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(area)[0];
    let inner = pane_inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    OverlayLayout {
        overlay,
        body: sections[0],
        footer: sections[1],
    }
}

pub fn settings_layout(area: Rect) -> SettingsLayout {
    let overlay = Layout::default()
        .margin(1)
        .constraints([Constraint::Min(1)])
        .split(area)[0];
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(overlay);

    SettingsLayout {
        overlay,
        tabs: sections[0],
        list: sections[1],
        footer: sections[2],
    }
}

pub fn add_to_playlist_layout(area: Rect) -> AddToPlaylistLayout {
    let overlay = centered_rect(area, 6, 76, 22);
    let inner = pane_inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    AddToPlaylistLayout {
        overlay,
        prompt: sections[0],
        list: sections[1],
        footer: sections[2],
    }
}

pub fn error_layout(area: Rect) -> ErrorLayout {
    let overlay = centered_rect(area, 4, 88, 12);
    let inner = pane_inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    ErrorLayout {
        overlay,
        body: sections[0],
    }
}

pub fn settings_tab_regions(area: Rect) -> Vec<(SettingsTab, Rect)> {
    let inner = pane_inner(area);
    if inner.width == 0 {
        return Vec::new();
    }

    let mut x = inner.left();
    let right = inner.right();
    let mut regions = Vec::with_capacity(SettingsTab::ALL.len());

    for (index, tab) in SettingsTab::ALL.iter().copied().enumerate() {
        let last_tab = index + 1 == SettingsTab::ALL.len();
        let start = x;

        let remaining_width = right.saturating_sub(x);
        if remaining_width == 0 {
            break;
        }
        x = x.saturating_add(
            SETTINGS_TAB_LEFT_PADDING
                .len()
                .min(remaining_width as usize) as u16,
        );

        let remaining_width = right.saturating_sub(x);
        if remaining_width == 0 {
            break;
        }
        x = x.saturating_add(tab.label().chars().count().min(remaining_width as usize) as u16);

        let remaining_width = right.saturating_sub(x);
        if remaining_width == 0 {
            regions.push((tab, Rect::new(start, inner.y, x.saturating_sub(start), 1)));
            break;
        }
        x = x.saturating_add(
            SETTINGS_TAB_RIGHT_PADDING
                .len()
                .min(remaining_width as usize) as u16,
        );
        regions.push((tab, Rect::new(start, inner.y, x.saturating_sub(start), 1)));

        let remaining_width = right.saturating_sub(x);
        if remaining_width == 0 || last_tab {
            break;
        }
        x = x.saturating_add(SETTINGS_TAB_DIVIDER.len().min(remaining_width as usize) as u16);
    }

    regions
}

pub fn settings_tab_at(area: Rect, column: u16, row: u16) -> Option<SettingsTab> {
    settings_tab_regions(area)
        .into_iter()
        .find_map(|(tab, rect)| {
            (column >= rect.x
                && column < rect.x.saturating_add(rect.width)
                && row >= rect.y
                && row < rect.y.saturating_add(rect.height))
            .then_some(tab)
        })
}

fn centered_rect(area: Rect, margin: u16, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(margin).min(max_width).max(1);
    let height = area.height.saturating_sub(margin).min(max_height).max(1);
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);

    horizontal[0]
}

fn content_summary_line_count(app: &AppState) -> u16 {
    if !matches!(app.route, Route::Search | Route::UserProfile) {
        return 0;
    }

    let view = app.current_content();
    let mut lines = 0;

    if !view.subtitle.trim().is_empty() {
        lines += 1;
    }

    let has_meta = !view.state_label.trim().is_empty()
        || view
            .help_message
            .as_ref()
            .is_some_and(|message| !message.trim().is_empty());
    if has_meta {
        lines += 1;
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;

    #[test]
    fn main_layout_handles_zero_viewport() {
        let app = AppState::new();
        assert!(main_layout_from_viewport(&app).is_none());
    }

    #[test]
    fn settings_tabs_map_across_inner_width() {
        let regions = settings_tab_regions(Rect::new(0, 0, 40, 3));
        assert_eq!(regions.len(), SettingsTab::ALL.len());
        assert_eq!(
            settings_tab_at(Rect::new(0, 0, 40, 3), 3, 1),
            Some(SettingsTab::Behavior)
        );
    }

    #[test]
    fn wide_header_split_uses_original_fixed_widths() {
        assert_eq!(
            wide_header_constraints(),
            [
                Constraint::Min(1),
                Constraint::Length(14),
                Constraint::Length(18)
            ]
        );
    }

    #[test]
    fn compact_header_split_uses_fifty_five_fifteen_thirty_ratio() {
        assert_eq!(
            compact_header_constraints(),
            [
                Constraint::Ratio(11, 20),
                Constraint::Ratio(3, 20),
                Constraint::Ratio(6, 20)
            ]
        );
    }

    #[test]
    fn header_components_have_no_spacing_between_them() {
        assert_eq!(HEADER_COMPONENT_SPACING, 0);
    }
}
