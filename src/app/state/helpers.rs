use chrono::Utc;
use crossterm::event::MouseEventKind;
use ratatui::layout::Rect;

use super::*;
use crate::ui::widgets::pane_inner;

pub(super) fn mock_playlists() -> Vec<SidebarPlaylist> {
    vec![
        SidebarPlaylist {
            urn: None,
            title: "Sunset Drive".to_string(),
            description: "Warm house and road-trip cuts".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Golden Hour", "Tycho", "Sunset Drive", "4:12"),
                ("La Mar", "Brijean", "Sunset Drive", "3:38"),
                ("Kites", "Bonobo", "Sunset Drive", "5:14"),
                ("Silk Route", "Ross From Friends", "Sunset Drive", "4:41"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Low Light".to_string(),
            description: "Late-night electronics and downtempo".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Night Bloom", "Tourist", "Low Light", "3:56"),
                ("Shoreline", "Ford.", "Low Light", "4:05"),
                ("Blink", "Four Tet", "Low Light", "4:24"),
                ("Shiver", "Catching Flies", "Low Light", "3:49"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Warehouse Mornings".to_string(),
            description: "Minimal grooves for long focus blocks".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Tracer", "Djoko", "Warehouse Mornings", "6:18"),
                ("Pebble", "Bicep", "Warehouse Mornings", "5:02"),
                ("Sunline", "Logic1000", "Warehouse Mornings", "4:47"),
                ("Lifted", "Mall Grab", "Warehouse Mornings", "5:23"),
            ]),
        },
        SidebarPlaylist {
            urn: None,
            title: "Cloud Sketches".to_string(),
            description: "Ambient drafts and instrumental loops".to_string(),
            creator: None,
            track_count: None,
            tracks: mock_track_rows(&[
                ("Paper Sky", "Helios", "Cloud Sketches", "3:18"),
                ("Still Water", "Hania Rani", "Cloud Sketches", "4:32"),
                ("Moss", "Kaitlyn Aurelia Smith", "Cloud Sketches", "5:07"),
                ("Resin", "Rival Consoles", "Cloud Sketches", "4:28"),
            ]),
        },
    ]
}

pub(super) fn mock_track_rows(items: &[(&str, &str, &str, &str)]) -> Vec<ContentRow> {
    items
        .iter()
        .map(|(title, artist, collection, length)| ContentRow {
            columns: [
                (*title).to_string(),
                (*artist).to_string(),
                (*collection).to_string(),
                (*length).to_string(),
            ],
        })
        .collect()
}

pub(super) fn pretty_activity_type(activity_type: &str) -> String {
    activity_type
        .replace('_', " ")
        .split_whitespace()
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn track_row_with_access(track: &TrackSummary) -> ContentRow {
    ContentRow {
        columns: [
            track.title.clone(),
            track.artist.clone(),
            track.access_label().to_string(),
            track.duration_label(),
        ],
    }
}

pub(super) fn playlist_row(playlist: &SoundcloudPlaylist) -> ContentRow {
    ContentRow {
        columns: [
            playlist.title.clone(),
            playlist.creator.clone(),
            playlist.track_count_label(),
            playlist.year_label(),
        ],
    }
}

pub(super) fn user_row(user: &UserSummary) -> ContentRow {
    ContentRow {
        columns: [
            user.username.clone(),
            user.followers_label(),
            user.spotlight_label(),
            "Profile".to_string(),
        ],
    }
}

pub(super) fn history_row(entry: &RecentlyPlayedEntry) -> ContentRow {
    ContentRow {
        columns: [
            entry.track.title.clone(),
            entry.track.artist.clone(),
            entry.context.clone(),
            relative_time_label(entry.played_at_epoch),
        ],
    }
}

pub(super) fn playlist_summary_subtitle(playlist: &SoundcloudPlaylist) -> String {
    if !playlist.description.trim().is_empty() {
        playlist.description.clone()
    } else {
        format!("By {} - {}", playlist.creator, playlist.track_count_label())
    }
}

pub(super) fn mouse_scroll_delta(kind: MouseEventKind) -> Option<isize> {
    match kind {
        MouseEventKind::ScrollDown => Some(1),
        MouseEventKind::ScrollUp => Some(-1),
        _ => None,
    }
}

pub(super) fn block_list_index_at_row(
    area: Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(pane_inner(area), column, row, len, selected, 0)
}

pub(super) fn plain_list_index_at_row(
    area: Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(area, column, row, len, selected, 0)
}

pub(super) fn table_index_at_row(
    area: Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
) -> Option<usize> {
    row_index_at(area, column, row, len, selected, 1)
}

pub(super) fn help_row(
    description: impl Into<String>,
    event: impl Into<String>,
    context: impl Into<String>,
) -> HelpRow {
    HelpRow {
        description: description.into(),
        event: event.into(),
        context: context.into(),
    }
}

fn relative_time_label(played_at_epoch: i64) -> String {
    let elapsed = (Utc::now().timestamp() - played_at_epoch).max(0);

    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", elapsed / 60),
        3_600..=86_399 => format!("{}h ago", elapsed / 3_600),
        86_400..=604_799 => format!("{}d ago", elapsed / 86_400),
        _ => format!("{}w ago", elapsed / 604_800),
    }
}

fn row_index_at(
    area: Rect,
    column: u16,
    row: u16,
    len: usize,
    selected: usize,
    header_rows: u16,
) -> Option<usize> {
    if len == 0 {
        return None;
    }

    if column < area.x || column >= area.x.saturating_add(area.width) {
        return None;
    }

    let start_row = area.y.saturating_add(header_rows);
    if row < start_row || row >= area.y.saturating_add(area.height) {
        return None;
    }

    let visible_rows = area.height.saturating_sub(header_rows) as usize;
    if visible_rows == 0 {
        return None;
    }

    let start_index = selected
        .min(len.saturating_sub(1))
        .saturating_sub(visible_rows.saturating_sub(1));
    let index = start_index + row.saturating_sub(start_row) as usize;

    (index < len).then_some(index)
}
