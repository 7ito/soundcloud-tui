use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::{
    app::{AppMode, AppState, AuthFocus, AuthStep, TextInput},
    ui::{
        geometry,
        widgets::{header_style, pane_block},
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    debug_assert_eq!(app.mode, AppMode::Auth);

    let block = pane_block("SoundCloud Onboarding", true, app);
    frame.render_widget(block, area);
    let layout = geometry::auth_layout(area);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Connect your SoundCloud account",
            header_style(app),
        )),
        Line::from("This Linux-first TUI keeps credentials and tokens on your machine."),
        Line::from(
            "Create a SoundCloud app, enter its credentials, then authorize in your browser.",
        ),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(header, layout.header);

    match app.auth.step {
        AuthStep::CheckingSession => render_checking(frame, layout.body, app),
        AuthStep::Credentials => render_credentials(frame, layout.body, app),
        AuthStep::WaitingForBrowser => render_waiting(frame, layout.body, app),
        AuthStep::ManualCallback => render_manual_callback(frame, layout.body, app),
    }

    render_footer(frame, layout.footer, app);
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
    .block(pane_block("Session", false, app))
    .wrap(Wrap { trim: true });

    frame.render_widget(body, area);
}

fn render_credentials(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = geometry::auth_credentials_layout(area);

    let instructions = Paragraph::new(vec![
        Line::from("1. Open your SoundCloud app dashboard and create or select an app."),
        Line::from(
            "2. Register the redirect URI shown below exactly in the SoundCloud app settings.",
        ),
        Line::from("3. Paste the client ID and client secret into the fields below."),
        Line::from("4. Press Save and Continue to start the OAuth browser flow."),
    ])
    .block(pane_block("Instructions", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, layout.instructions);

    render_input(
        frame,
        layout.client_id,
        "Client ID",
        &app.auth.form.client_id,
        app.auth.focus == AuthFocus::ClientId,
        false,
        app,
    );
    render_input(
        frame,
        layout.client_secret,
        "Client Secret",
        &app.auth.form.client_secret,
        app.auth.focus == AuthFocus::ClientSecret,
        true,
        app,
    );
    render_input(
        frame,
        layout.redirect_uri,
        "Redirect URI",
        &app.auth.form.redirect_uri,
        app.auth.focus == AuthFocus::RedirectUri,
        false,
        app,
    );
    render_button(
        frame,
        layout.open_apps,
        "Open SoundCloud Apps Page",
        app.auth.focus == AuthFocus::OpenAppsPage,
        app,
    );
    render_button(
        frame,
        layout.save_and_continue,
        "Save and Continue",
        app.auth.focus == AuthFocus::SaveAndContinue,
        app,
    );

    let reminder = Paragraph::new(vec![
        Line::from("Credentials are stored locally in ~/.config/soundcloud-tui/credentials.toml."),
        Line::from("Click a field to place the cursor, or use Tab/Up/Down to move focus and Enter on buttons."),
        Line::from("Paste works with terminal paste shortcuts and with Ctrl+V when clipboard access is available."),
    ])
    .block(pane_block("Local Storage", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(reminder, layout.reminder);
}

fn render_waiting(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = geometry::auth_waiting_layout(area);

    let instructions = Paragraph::new(vec![
        Line::from("The credentials were saved. Next, approve access in your browser."),
        Line::from("SoundCloud will redirect back to your localhost callback URI when authorization finishes."),
        Line::from("If automatic capture fails, switch to manual callback mode and paste the full redirected URL."),
    ])
    .block(pane_block("Authorize", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, layout.instructions);

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
    .block(pane_block("Browser", false, app))
    .wrap(Wrap { trim: false });
    frame.render_widget(auth_url_widget, layout.browser);

    render_button(
        frame,
        layout.open_browser,
        "Open Browser Again",
        app.auth.focus == AuthFocus::OpenBrowser,
        app,
    );
    render_button(
        frame,
        layout.paste_callback,
        "Paste Callback URL",
        app.auth.focus == AuthFocus::PasteCallback,
        app,
    );
    render_button(
        frame,
        layout.back_to_credentials,
        "Back to Credentials",
        app.auth.focus == AuthFocus::BackToCredentials,
        app,
    );

    let status = Paragraph::new(vec![
        Line::from(format!("State: {}", app.loading_label())),
        Line::from(
            "If your browser did not open automatically, click the button above or copy the URL into it manually.",
        ),
    ])
    .block(pane_block("Status", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(status, layout.status);
}

fn render_manual_callback(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let layout = geometry::auth_manual_callback_layout(area);

    let instructions = Paragraph::new(vec![
        Line::from("Automatic callback capture could not finish the flow."),
        Line::from("After approving access in the browser, copy the full callback URL from the address bar."),
        Line::from("Paste that URL into the field below and submit it to complete the token exchange."),
    ])
    .block(pane_block("Manual Callback", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(instructions, layout.instructions);

    render_input(
        frame,
        layout.callback_input,
        "Callback URL",
        &app.auth.callback_input,
        app.auth.focus == AuthFocus::CallbackInput,
        false,
        app,
    );
    render_button(
        frame,
        layout.submit_callback,
        "Submit Callback URL",
        app.auth.focus == AuthFocus::SubmitCallback,
        app,
    );
    render_button(
        frame,
        layout.back_to_browser,
        "Back to Browser Step",
        app.auth.focus == AuthFocus::BackToBrowser,
        app,
    );

    let help = Paragraph::new(vec![
        Line::from("Accepted input: the full callback URL or just the raw query string containing code and state."),
        Line::from("Example: http://127.0.0.1:8974/callback?code=...&state=..."),
    ])
    .block(pane_block("Accepted Formats", false, app))
    .wrap(Wrap { trim: true });
    frame.render_widget(help, layout.help);
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
        .block(pane_block("Footer", false, app))
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
    app: &AppState,
) {
    let display_value = input.display_value(masked);
    let block = pane_block(title, active, app);
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

fn render_button(frame: &mut Frame<'_>, area: Rect, label: &str, active: bool, app: &AppState) {
    let button = Paragraph::new(Line::from(Span::styled(label, header_style(app))))
        .block(pane_block(label, active, app))
        .centered();
    frame.render_widget(button, area);
}
