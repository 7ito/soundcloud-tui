use ratatui::{
    layout::{Constraint, Direction, Flex, Layout, Rect},
    text::{Line, Text},
    widgets::{Clear, Paragraph, Wrap},
    Frame,
};

use crate::{app::AppState, ui::widgets::pane_block};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(error) = app.error_modal.as_ref() else {
        return;
    };

    let overlay = centered_rect(area);
    let block = pane_block(error.title.as_str(), true);
    let inner = block.inner(overlay);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body = Paragraph::new(Text::from(vec![
        Line::from(error.message.as_str()),
        Line::from(""),
        Line::from("Press Esc or Enter to dismiss."),
    ]))
    .wrap(Wrap { trim: true });

    frame.render_widget(Clear, overlay);
    frame.render_widget(block, overlay);
    frame.render_widget(body, sections[0]);
}

fn centered_rect(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(88).max(1);
    let height = area.height.saturating_sub(4).min(12).max(1);
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
