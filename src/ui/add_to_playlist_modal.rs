use ratatui::{
    Frame,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    ui::widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block, selected_row_style},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(modal) = app.add_to_playlist_modal.as_ref() else {
        return;
    };

    let overlay = centered_rect(area);
    let block = pane_block("Add Track To Playlist", true, app);
    let inner = block.inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let prompt = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("Choose a playlist for: "),
            Span::styled(modal.track.title.as_str(), header_style(app)),
        ]),
        Line::from(Span::styled(
            modal.track.artist.as_str(),
            Style::default().fg(app.theme().inactive),
        )),
    ])
    .wrap(Wrap { trim: true });

    frame.render_widget(Clear, overlay);
    frame.render_widget(block, overlay);
    frame.render_widget(prompt, sections[0]);

    if app.playlists.is_empty() {
        let empty = Paragraph::new("No playlists are available yet.")
            .style(Style::default().fg(app.theme().inactive))
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, sections[1]);
    } else {
        let items = app
            .playlists
            .iter()
            .map(|playlist| ListItem::new(playlist.title.clone()))
            .collect::<Vec<_>>();
        let list = List::new(items)
            .highlight_style(selected_row_style(app))
            .highlight_symbol(HIGHLIGHT_SYMBOL);
        let mut state = ListState::default();
        state.select(Some(
            modal
                .selected_playlist
                .min(app.playlists.len().saturating_sub(1)),
        ));
        frame.render_stateful_widget(list, sections[1], &mut state);
    }

    let footer = Paragraph::new("Enter add | q cancel | j/k or arrows move | H/M/L jump")
        .style(Style::default().fg(app.theme().inactive))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, sections[2]);
}

fn centered_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(6).min(76).max(1);
    let height = area.height.saturating_sub(6).min(22).max(1);
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
