use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::{
    app::AppState,
    ui::{
        theme::Theme,
        widgets::{header_style, pane_block, selected_row_style, HIGHLIGHT_SYMBOL},
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let overlay = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(area)[0];
    let block = pane_block("Queue (press Esc to go back)", true);
    let inner = block.inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let rows = app.queue_overlay_rows();
    let header =
        Row::new(["Title", "Artist", "State", "Length"].map(Cell::from)).style(header_style());
    let table_rows = rows.iter().map(|row| {
        Row::new(
            row.columns
                .iter()
                .cloned()
                .map(Cell::from)
                .collect::<Vec<_>>(),
        )
    });
    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(28),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
    ];
    let table = Table::new(table_rows, widths)
        .header(header)
        .highlight_symbol(HIGHLIGHT_SYMBOL)
        .row_highlight_style(selected_row_style());
    let footer = Paragraph::new(Line::from(
        "Enter play | d remove | Esc close | j/k or arrows move",
    ))
    .style(Style::default().fg(Theme::default().muted));

    frame.render_widget(Clear, area);
    frame.render_widget(block, overlay);

    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No queued tracks.").style(Style::default().fg(Theme::default().muted)),
            sections[0],
        );
    } else {
        let mut state = TableState::default();
        state.select(app.queue_overlay_selection());
        frame.render_stateful_widget(table, sections[0], &mut state);
    }

    frame.render_widget(footer, sections[1]);
}
