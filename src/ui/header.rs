use ratatui::{
    Frame,
    layout::{Alignment, Position, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::{AppState, Focus},
    ui::widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block},
};

pub fn render_search(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = pane_block("Search", app.focus == Focus::Search);
    let inner = block.inner(area);
    let search = Paragraph::new(Line::from(vec![
        Span::styled(HIGHLIGHT_SYMBOL, header_style()),
        Span::raw(app.search_query.as_str()),
    ]))
    .block(block);

    frame.render_widget(search, area);

    if app.focus == Focus::Search {
        let cursor_x = area
            .x
            .saturating_add(inner.x.saturating_sub(area.x))
            .saturating_add(HIGHLIGHT_SYMBOL.chars().count() as u16)
            .saturating_add(app.search_cursor as u16)
            .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
        frame.set_cursor_position(Position::new(cursor_x, inner.y));
    }
}

pub fn render_help(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let help = Paragraph::new("?")
        .alignment(Alignment::Left)
        .block(pane_block("Help", app.show_help));

    frame.render_widget(help, area);
}
