use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    ui::{geometry, widgets::pane_block},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(error) = app.error_modal.as_ref() else {
        return;
    };

    let layout = geometry::error_layout(area);
    let overlay = layout.overlay;
    let block = pane_block(error.title.as_str(), true, app);
    let body = Paragraph::new(Text::from(vec![
        Line::from(error.message.as_str()),
        Line::from(""),
        Line::from("Press Esc or Enter to dismiss."),
    ]))
    .style(Style::default().fg(app.theme().error_text))
    .wrap(Wrap { trim: true });

    frame.render_widget(Clear, overlay);
    frame.render_widget(block, overlay);
    frame.render_widget(body, layout.body);
}
