use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    app::{AppState, Focus, Route},
    ui::widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block, selected_row_style},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let view = app.current_content();
    let block = pane_block(view.title.as_str(), app.focus == Focus::Content);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    let content_area = if app.route == Route::Search {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        let summary = Paragraph::new(Line::from(view.subtitle.clone())).wrap(Wrap { trim: true });
        frame.render_widget(summary, chunks[0]);
        chunks[1]
    } else {
        inner
    };

    if view.rows.is_empty() {
        let empty = Paragraph::new(view.empty_message).wrap(Wrap { trim: true });
        frame.render_widget(empty, content_area);
    } else {
        let rows = view.rows.iter().map(|row| {
            Row::new(
                row.columns
                    .iter()
                    .cloned()
                    .map(Cell::from)
                    .collect::<Vec<_>>(),
            )
        });
        let header = Row::new(view.columns.map(Cell::from)).style(header_style());
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(30),
                Constraint::Percentage(26),
                Constraint::Percentage(30),
                Constraint::Percentage(14),
            ],
        )
        .header(header)
        .row_highlight_style(selected_row_style())
        .column_spacing(1)
        .highlight_symbol(HIGHLIGHT_SYMBOL);

        let mut state = TableState::default();
        state.select(Some(app.selected_content.min(view.rows.len() - 1)));
        frame.render_stateful_widget(table, content_area, &mut state);
    }
}
