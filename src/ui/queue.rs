use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Style,
    text::Line,
    widgets::{Cell, Clear, Paragraph, Row, Table, TableState},
};

use crate::{
    app::AppState,
    ui::{
        geometry,
        widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block, selected_row_style},
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = geometry::queue_layout(area);
    let overlay = layout.overlay;
    let block = pane_block("Queue (press Esc to go back)", true, app);
    let rows = app.queue_overlay_rows();
    let header =
        Row::new(["Title", "Artist", "State", "Length"].map(Cell::from)).style(header_style(app));
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
        .row_highlight_style(selected_row_style(app));
    let footer = Paragraph::new(Line::from(
        "Enter play | d remove | Esc close | j/k or arrows move",
    ))
    .style(Style::default().fg(app.theme().inactive));

    frame.render_widget(Clear, area);
    frame.render_widget(block, overlay);

    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No queued tracks.").style(Style::default().fg(app.theme().inactive)),
            layout.body,
        );
    } else {
        let mut state = TableState::default();
        state.select(app.queue_overlay_selection());
        frame.render_stateful_widget(table, layout.body, &mut state);
    }

    frame.render_widget(footer, layout.footer);
}
