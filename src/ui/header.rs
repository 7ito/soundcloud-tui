use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::{AppState, Focus},
    ui::widgets::{header_style, pane_block},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(82), Constraint::Percentage(18)])
        .split(area);

    let search = Paragraph::new(Line::from(vec![
        Span::styled("> ", header_style()),
        Span::raw(app.search_query.as_str()),
        Span::raw("  (Enter searches SoundCloud | 1/2/3 switch search tables)"),
    ]))
    .block(pane_block("Search", app.focus == Focus::Search));

    let help = Paragraph::new(vec![
        Line::from(format!("Focus: {}", app.focus.label())),
        Line::from(app.header_help_label()),
    ])
    .block(pane_block("Help", false));

    frame.render_widget(search, chunks[0]);
    frame.render_widget(help, chunks[1]);

    if app.focus == Focus::Search {
        let cursor_x = chunks[0]
            .x
            .saturating_add(3)
            .saturating_add(app.search_cursor as u16)
            .min(
                chunks[0]
                    .x
                    .saturating_add(chunks[0].width.saturating_sub(2)),
            );
        frame.set_cursor_position(Position::new(cursor_x, chunks[0].y.saturating_add(1)));
    }
}
