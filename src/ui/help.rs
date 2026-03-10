use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Cell, Clear, Paragraph, Row, Table},
};

use crate::{
    app::{AppState, HelpRow},
    ui::{
        theme::Theme,
        widgets::{header_style, pane_block},
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let overlay = Layout::default()
        .constraints([Constraint::Min(1)])
        .margin(1)
        .split(area)[0];
    let block = pane_block("Help (press <Esc> to go back)", true);
    let inner = block.inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let rows = app.help_rows();
    let visible_rows = sections[0].height.saturating_sub(1).max(1) as usize;
    let scroll = app.help_scroll.min(rows.len().saturating_sub(visible_rows));
    let widths = [
        Constraint::Percentage(41),
        Constraint::Percentage(31),
        Constraint::Percentage(28),
    ];
    let header =
        Row::new(["Description", "Event", "Context"].map(Cell::from)).style(header_style());
    let table_rows = rows
        .iter()
        .skip(scroll)
        .take(visible_rows)
        .map(|row| render_row(row, sections[0].width));
    let visible_end = (scroll + visible_rows).min(rows.len());
    let footer = Paragraph::new(Line::from(format!(
        "{}-{} of {}",
        if rows.is_empty() { 0 } else { scroll + 1 },
        visible_end,
        rows.len()
    )))
    .style(ratatui::style::Style::default().fg(Theme::default().muted));
    let table = Table::new(table_rows, widths).header(header);

    frame.render_widget(Clear, area);
    frame.render_widget(block, overlay);
    frame.render_widget(table, sections[0]);
    frame.render_widget(footer, sections[1]);
}

fn render_row(row: &HelpRow, width: u16) -> Row<'static> {
    let [description_width, event_width, context_width] = column_widths(width as usize);

    Row::new([
        Cell::from(truncate(row.description, description_width)),
        Cell::from(truncate(row.event, event_width)),
        Cell::from(truncate(row.context, context_width)),
    ])
}

fn column_widths(total_width: usize) -> [usize; 3] {
    let description_width = ((total_width * 41) / 100).saturating_sub(2);
    let remaining_after_description = total_width.saturating_sub(description_width);
    let event_width = ((total_width * 31) / 100)
        .min(remaining_after_description)
        .saturating_sub(2);
    let context_width = total_width
        .saturating_sub(description_width)
        .saturating_sub(event_width)
        .saturating_sub(2);

    [description_width, event_width, context_width]
}

fn truncate(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if text.chars().count() <= max_width {
        return text.to_string();
    }

    if max_width <= 1 {
        return "…".to_string();
    }

    let mut truncated = text
        .chars()
        .take(max_width.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::app::AppState;

    #[test]
    fn column_widths_do_not_underflow_on_tiny_widths() {
        for width in 0..8 {
            let [description, event, context] = column_widths(width);
            assert!(description + event + context <= width);
        }
    }

    #[test]
    fn help_render_handles_small_viewports() {
        let backend = TestBackend::new(12, 6);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::new();
        app.show_help = true;

        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .expect("help render should not panic on small viewports");
    }
}
