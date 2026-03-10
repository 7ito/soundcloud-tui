use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    app::{AppState, Focus},
    ui::widgets::{header_style, pane_block, selected_row_style},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let view = app.current_content();
    let block = pane_block(view.title.as_str(), app.focus == Focus::Content);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(1)])
        .split(inner);

    let mut summary_lines = vec![
        Line::from(view.subtitle.clone()),
        Line::from(format!("State: {}", view.state_label)),
        Line::from(app.auth_summary.as_str()),
        Line::from(format!(
            "Focus {} | Route {} | Viewport {}x{}",
            app.focus.label(),
            app.route_title(),
            app.viewport.width,
            app.viewport.height
        )),
    ];
    if let Some(help) = &view.help_message {
        summary_lines.push(Line::from(help.clone()));
    }

    let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: true });
    frame.render_widget(summary, chunks[0]);

    if view.rows.is_empty() {
        let mut empty_lines = vec![
            Line::from(view.state_label.clone()),
            Line::from(""),
            Line::from(view.empty_message),
        ];
        if let Some(help) = &view.help_message {
            empty_lines.push(Line::from(""));
            empty_lines.push(Line::from(help.clone()));
        }

        let empty = Paragraph::new(empty_lines).wrap(Wrap { trim: true });
        frame.render_widget(empty, chunks[1]);
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
        .highlight_symbol("> ");

        let mut state = TableState::default();
        state.select(Some(app.selected_content.min(view.rows.len() - 1)));
        frame.render_stateful_widget(table, chunks[1], &mut state);
    }
}
