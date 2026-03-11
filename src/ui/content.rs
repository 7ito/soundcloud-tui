use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, TableState, Wrap},
};

use crate::{
    app::{AppState, Focus, Route},
    ui::geometry,
    ui::widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block, selected_row_style},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let view = app.current_content();
    let block = pane_block(view.title.as_str(), app.focus == Focus::Content, app);
    let layout = geometry::content_layout(area, app);
    let visible_columns = visible_column_indices(app.route);

    frame.render_widget(block, area);

    let summary_lines = if matches!(app.route, Route::Search | Route::UserProfile) {
        let mut lines = Vec::new();

        if !view.subtitle.trim().is_empty() {
            lines.push(Line::from(view.subtitle.clone()));
        }

        let mut meta = Vec::new();
        if !view.state_label.trim().is_empty() {
            meta.push(Span::styled(view.state_label.clone(), header_style(app)));
        }
        if let Some(help_message) = view
            .help_message
            .as_ref()
            .filter(|message| !message.trim().is_empty())
        {
            if !meta.is_empty() {
                meta.push(Span::raw(" | "));
            }
            meta.push(Span::styled(
                help_message.clone(),
                Style::default().fg(app.theme().inactive),
            ));
        }
        if !meta.is_empty() {
            lines.push(Line::from(meta));
        }

        lines
    } else {
        Vec::new()
    };

    let content_area = if let Some(summary_area) = layout.summary {
        let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: true });
        frame.render_widget(summary, summary_area);
        layout.body
    } else {
        layout.body
    };

    if view.rows.is_empty() {
        let empty = Paragraph::new(view.empty_message).wrap(Wrap { trim: true });
        frame.render_widget(empty, content_area);
    } else {
        let rows = view.rows.iter().map(|row| {
            Row::new(
                visible_columns
                    .iter()
                    .map(|&index| row.columns[index].clone())
                    .map(Cell::from)
                    .collect::<Vec<_>>(),
            )
        });
        let header = Row::new(
            visible_columns
                .iter()
                .map(|&index| view.columns[index])
                .map(Cell::from)
                .collect::<Vec<_>>(),
        )
        .style(header_style(app));
        let table = Table::new(rows, column_constraints(visible_columns))
            .header(header)
            .row_highlight_style(selected_row_style(app))
            .column_spacing(1)
            .highlight_symbol(HIGHLIGHT_SYMBOL);

        let mut state = TableState::default();
        state.select(Some(app.selected_content.min(view.rows.len() - 1)));
        frame.render_stateful_widget(table, content_area, &mut state);
    }
}

fn visible_column_indices(route: Route) -> &'static [usize] {
    match route {
        Route::Feed | Route::LikedSongs => &[0, 1, 3],
        _ => &[0, 1, 2, 3],
    }
}

fn column_constraints(visible_columns: &[usize]) -> Vec<Constraint> {
    match visible_columns.len() {
        3 => vec![
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
        _ => vec![
            Constraint::Percentage(30),
            Constraint::Percentage(26),
            Constraint::Percentage(30),
            Constraint::Percentage(14),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::visible_column_indices;
    use crate::app::Route;

    #[test]
    fn feed_hides_source_column() {
        assert_eq!(visible_column_indices(Route::Feed), &[0, 1, 3]);
    }

    #[test]
    fn liked_songs_hides_access_column() {
        assert_eq!(visible_column_indices(Route::LikedSongs), &[0, 1, 3]);
    }

    #[test]
    fn other_routes_keep_all_columns() {
        assert_eq!(visible_column_indices(Route::Playlist), &[0, 1, 2, 3]);
    }
}
