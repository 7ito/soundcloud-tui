use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    ui::{
        geometry,
        widgets::{header_style, pane_block},
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(modal) = app.logout_confirm_modal.as_ref() else {
        return;
    };

    let layout = geometry::logout_confirm_layout(area);
    let overlay = layout.overlay;
    let body = Paragraph::new(body_text(app, modal))
        .style(Style::default().fg(app.theme().text))
        .wrap(Wrap { trim: true });
    let footer = Paragraph::new("Enter log out | Esc cancel | Mouse click buttons")
        .style(Style::default().fg(app.theme().inactive))
        .wrap(Wrap { trim: true });

    frame.render_widget(Clear, overlay);
    frame.render_widget(pane_block("Confirm Log Out", true, app), overlay);
    frame.render_widget(body, layout.body);
    render_button(frame, layout.cancel_button, "Cancel", false, app);
    render_button(frame, layout.confirm_button, "Log Out", true, app);
    frame.render_widget(footer, layout.footer);
}

fn body_text<'a>(app: &AppState, modal: &'a crate::app::LogoutConfirmModal) -> Text<'a> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::raw("Log out of "),
        Span::styled(
            modal
                .username
                .as_deref()
                .unwrap_or("this SoundCloud account"),
            header_style(app),
        ),
        Span::raw("?"),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(
        "This clears the saved SoundCloud session tokens on this machine.",
    ));
    lines.push(Line::from(
        "Your app credentials stay saved so you can sign in again quickly.",
    ));

    if modal.discard_unsaved_changes {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Unsaved settings changes will be discarded.",
            Style::default().fg(app.theme().hint),
        )));
    }

    Text::from(lines)
}

fn render_button(frame: &mut Frame<'_>, area: Rect, label: &str, primary: bool, app: &AppState) {
    let button = Paragraph::new(Line::from(Span::styled(label, header_style(app))))
        .block(pane_block(label, primary, app))
        .centered();
    frame.render_widget(button, area);
}
