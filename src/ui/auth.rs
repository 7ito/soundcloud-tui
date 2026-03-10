use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::{
    app::{AppMode, AppState, AuthFocus, AuthStep, TextInput},
    ui::widgets::{header_style, pane_block},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    debug_assert_eq!(app.mode, AppMode::Auth);

    let block = pane_block("SoundCloud Onboarding", true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(inner);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Connect your SoundCloud account",
            header_style(),
        )),
        Line::from("This Linux-first TUI keeps credentials and tokens on your machine."),
        Line::from(
            "Create a SoundCloud app, enter its credentials, then authorize in your browser.",
        ),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(header, sections[0]);

    match app.auth.step {
        AuthStep::CheckingSession => render_checking(frame, sections[1], app),
        AuthStep::Credentials => render_credentials(frame, sections[1], app),
        AuthStep::WaitingForBrowser => render_waiting(frame, sections[1], app),
        AuthStep::ManualCallback => render_manual_callback(frame, sections[1], app),
    }

    render_footer(frame, sections[2], app);
}

fn render_checking(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let dots = match app.tick_count % 4 {
        0 => "",
        1 => ".",
        2 => "..",
        _ => "...",
    };

    let body = Paragraph::new(vec![
        Line::from(format!("Checking for an existing SoundCloud session{dots}")),
        Line::from(
            "If saved credentials and tokens are valid, the player shell opens automatically.",
        ),
        Line::from("Otherwise you will land on the credential form below."),
    ])
    .block(pane_block("Session", false))
    .wrap(Wrap { trim: true });

    frame.render_widget(body, area);
}

fn render_credentials(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let instructions = Paragraph::new(vec![
        Line::from("1. Open your SoundCloud app dashboard and create or select an app."),
        Line::from(
            "2. Register the redirect URI shown below exactly in the SoundCloud app settings.",
        ),
        Line::from("3. Paste the client ID and client secret into the fields below."),
        Line::from("4. Press Save and Continue to start the OAuth browser flow."),
    ])
    .block(pane_block("Instructions", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, rows[0]);

    render_input(
        frame,
        rows[1],
        "Client ID",
        &app.auth.form.client_id,
        app.auth.focus == AuthFocus::ClientId,
        false,
    );
    render_input(
        frame,
        rows[2],
        "Client Secret",
        &app.auth.form.client_secret,
        app.auth.focus == AuthFocus::ClientSecret,
        true,
    );
    render_input(
        frame,
        rows[3],
        "Redirect URI",
        &app.auth.form.redirect_uri,
        app.auth.focus == AuthFocus::RedirectUri,
        false,
    );

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[4]);
    render_button(
        frame,
        buttons[0],
        "Open SoundCloud Apps Page",
        app.auth.focus == AuthFocus::OpenAppsPage,
    );
    render_button(
        frame,
        buttons[1],
        "Save and Continue",
        app.auth.focus == AuthFocus::SaveAndContinue,
    );

    let reminder = Paragraph::new(vec![
        Line::from("Credentials are stored locally in ~/.config/soundcloud-tui/credentials.toml."),
        Line::from("Use Tab or Up/Down to move focus, type into fields, and press Enter on buttons."),
        Line::from("Paste works with terminal paste shortcuts and with Ctrl+V when clipboard access is available."),
    ])
    .block(pane_block("Local Storage", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(reminder, rows[5]);
}

fn render_waiting(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let instructions = Paragraph::new(vec![
        Line::from("The credentials were saved. Next, approve access in your browser."),
        Line::from("SoundCloud will redirect back to your localhost callback URI when authorization finishes."),
        Line::from("If automatic capture fails, switch to manual callback mode and paste the full redirected URL."),
    ])
    .block(pane_block("Authorize", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, rows[0]);

    let auth_url = app
        .auth
        .auth_url
        .as_deref()
        .unwrap_or("Authorization URL unavailable");
    let auth_url_widget = Paragraph::new(vec![
        Line::from("Authorization URL:"),
        Line::from(auth_url),
        Line::from(""),
        Line::from(
            "Approve in the browser, then return here. The app is listening for the callback.",
        ),
    ])
    .block(pane_block("Browser", false))
    .wrap(Wrap { trim: false });
    frame.render_widget(auth_url_widget, rows[1]);

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(rows[2]);
    render_button(
        frame,
        buttons[0],
        "Open Browser Again",
        app.auth.focus == AuthFocus::OpenBrowser,
    );
    render_button(
        frame,
        buttons[1],
        "Paste Callback URL",
        app.auth.focus == AuthFocus::PasteCallback,
    );
    render_button(
        frame,
        buttons[2],
        "Back to Credentials",
        app.auth.focus == AuthFocus::BackToCredentials,
    );

    let status = Paragraph::new(vec![
        Line::from(format!("State: {}", app.loading_label())),
        Line::from(
            "If your browser did not open automatically, copy the URL above into it manually.",
        ),
    ])
    .block(pane_block("Status", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(status, rows[3]);
}

fn render_manual_callback(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(area);

    let instructions = Paragraph::new(vec![
        Line::from("Automatic callback capture could not finish the flow."),
        Line::from("After approving access in the browser, copy the full callback URL from the address bar."),
        Line::from("Paste that URL into the field below and submit it to complete the token exchange."),
    ])
    .block(pane_block("Manual Callback", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, rows[0]);

    render_input(
        frame,
        rows[1],
        "Callback URL",
        &app.auth.callback_input,
        app.auth.focus == AuthFocus::CallbackInput,
        false,
    );

    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);
    render_button(
        frame,
        buttons[0],
        "Submit Callback URL",
        app.auth.focus == AuthFocus::SubmitCallback,
    );
    render_button(
        frame,
        buttons[1],
        "Back to Browser Step",
        app.auth.focus == AuthFocus::BackToBrowser,
    );

    let help = Paragraph::new(vec![
        Line::from("Accepted input: the full callback URL or just the raw query string containing code and state."),
        Line::from("Example: http://127.0.0.1:8974/callback?code=...&state=..."),
    ])
    .block(pane_block("Accepted Formats", false))
    .wrap(Wrap { trim: true });
    frame.render_widget(help, rows[3]);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let mut lines = vec![Line::from(format!("Info: {}", app.auth.info))];

    if let Some(error) = &app.auth.error {
        lines.push(Line::from(Span::styled(
            format!("Error: {error}"),
            Style::default().fg(Color::Red),
        )));
    } else {
        lines.push(Line::from(format!("Status: {}", app.status)));
    }

    lines.push(Line::from("Global shortcut: Ctrl+C quits the app."));

    let footer = Paragraph::new(lines)
        .block(pane_block("Footer", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, area);
}

fn render_input(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    input: &TextInput,
    active: bool,
    masked: bool,
) {
    let display_value = input.display_value(masked);
    let block = pane_block(title, active);
    let inner = block.inner(area);

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(display_value.as_str()), inner);

    if active {
        let cursor_x = inner
            .x
            .saturating_add(input.cursor as u16)
            .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
        frame.set_cursor_position(Position::new(cursor_x, inner.y));
    }
}

fn render_button(frame: &mut Frame<'_>, area: Rect, label: &str, active: bool) {
    let button = Paragraph::new(Line::from(Span::styled(label, header_style())))
        .block(pane_block(label, active))
        .centered();
    frame.render_widget(button, area);
}
