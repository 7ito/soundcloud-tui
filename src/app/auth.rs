use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::process::Command;

use crate::{config::credentials::Credentials, soundcloud::auth::AuthorizationRequest};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AppMode {
    Auth,
    Main,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AuthStep {
    CheckingSession,
    Credentials,
    WaitingForBrowser,
    ManualCallback,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AuthFocus {
    OpenAppsPage,
    ClientId,
    ClientSecret,
    RedirectUri,
    SaveAndContinue,
    OpenBrowser,
    PasteCallback,
    BackToCredentials,
    CallbackInput,
    SubmitCallback,
    BackToBrowser,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AuthIntent {
    OpenAppsPage,
    SaveAndContinue,
    OpenBrowser,
    ShowManualCallback,
    BackToCredentials,
    SubmitManualCallback,
    BackToBrowser,
}

#[derive(Debug, Clone)]
pub struct AuthState {
    pub step: AuthStep,
    pub focus: AuthFocus,
    pub form: CredentialsForm,
    pub callback_input: TextInput,
    pub auth_url: Option<String>,
    pub pending_authorization: Option<AuthorizationRequest>,
    pub info: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CredentialsForm {
    pub client_id: TextInput,
    pub client_secret: TextInput,
    pub redirect_uri: TextInput,
}

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl AuthState {
    pub fn new(prefill: Credentials) -> Self {
        Self {
            step: AuthStep::Credentials,
            focus: AuthFocus::ClientId,
            form: CredentialsForm::from(prefill),
            callback_input: TextInput::default(),
            auth_url: None,
            pending_authorization: None,
            info: "Enter your SoundCloud app credentials to continue.".to_string(),
            error: None,
        }
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn set_info(&mut self, message: impl Into<String>) {
        self.info = message.into();
    }

    pub fn set_checking_session(&mut self) {
        self.step = AuthStep::CheckingSession;
        self.focus = AuthFocus::OpenAppsPage;
        self.info = "Checking for an existing SoundCloud session...".to_string();
        self.error = None;
    }

    pub fn set_waiting_for_browser(&mut self, request: AuthorizationRequest) {
        self.step = AuthStep::WaitingForBrowser;
        self.focus = AuthFocus::OpenBrowser;
        self.auth_url = Some(request.authorize_url.clone());
        self.pending_authorization = Some(request);
        self.info = "Authorize the app in your browser, then return here.".to_string();
        self.error = None;
        self.callback_input = TextInput::default();
    }

    pub fn show_manual_callback(&mut self, message: impl Into<String>) {
        self.step = AuthStep::ManualCallback;
        self.focus = AuthFocus::CallbackInput;
        self.info = message.into();
    }

    pub fn back_to_credentials(&mut self) {
        self.step = AuthStep::Credentials;
        self.focus = AuthFocus::ClientId;
        self.auth_url = None;
        self.pending_authorization = None;
        self.callback_input = TextInput::default();
        self.info = "Update your SoundCloud app credentials and continue.".to_string();
    }

    pub fn credentials(&self) -> Credentials {
        Credentials {
            client_id: self.form.client_id.value.clone(),
            client_secret: self.form.client_secret.value.clone(),
            redirect_uri: self.form.redirect_uri.value.clone(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<AuthIntent> {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focus = self.next_focus();
                None
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = self.previous_focus();
                None
            }
            KeyCode::Enter => self.submit_focus(),
            KeyCode::Left if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.move_left();
                }
                None
            }
            KeyCode::Right if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.move_right();
                }
                None
            }
            KeyCode::Home if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.move_home();
                }
                None
            }
            KeyCode::End if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.move_end();
                }
                None
            }
            KeyCode::Backspace if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.backspace();
                }
                None
            }
            KeyCode::Delete if self.active_input_mut().is_some() => {
                if let Some(input) = self.active_input_mut() {
                    input.delete();
                }
                None
            }
            KeyCode::Char('v')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && self.active_input_mut().is_some() =>
            {
                self.paste_clipboard();
                None
            }
            KeyCode::Insert
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    && self.active_input_mut().is_some() =>
            {
                self.paste_clipboard();
                None
            }
            KeyCode::Char(ch)
                if key.modifiers == KeyModifiers::NONE && self.active_input_mut().is_some() =>
            {
                if let Some(input) = self.active_input_mut() {
                    input.insert(ch);
                }
                None
            }
            _ => None,
        }
    }

    pub fn paste_text(&mut self, text: &str) {
        let sanitized = text.replace(['\r', '\n'], "");
        if let Some(input) = self.active_input_mut() {
            input.insert_str(&sanitized);
        }
        self.clear_error();
    }

    pub fn click_focus(&mut self, focus: AuthFocus, cursor: Option<usize>) -> Option<AuthIntent> {
        self.focus = focus;
        if let Some(cursor) = cursor {
            if let Some(input) = self.active_input_mut() {
                input.set_cursor(cursor);
            }
            None
        } else {
            self.focused_action()
        }
    }

    fn submit_focus(&mut self) -> Option<AuthIntent> {
        if let Some(intent) = self.focused_action() {
            Some(intent)
        } else {
            self.focus = self.next_focus();
            None
        }
    }

    fn focused_action(&self) -> Option<AuthIntent> {
        match self.focus {
            AuthFocus::OpenAppsPage => Some(AuthIntent::OpenAppsPage),
            AuthFocus::SaveAndContinue => Some(AuthIntent::SaveAndContinue),
            AuthFocus::OpenBrowser => Some(AuthIntent::OpenBrowser),
            AuthFocus::PasteCallback => Some(AuthIntent::ShowManualCallback),
            AuthFocus::BackToCredentials => Some(AuthIntent::BackToCredentials),
            AuthFocus::SubmitCallback => Some(AuthIntent::SubmitManualCallback),
            AuthFocus::BackToBrowser => Some(AuthIntent::BackToBrowser),
            _ => None,
        }
    }

    fn active_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.focus {
            AuthFocus::ClientId if self.step == AuthStep::Credentials => {
                Some(&mut self.form.client_id)
            }
            AuthFocus::ClientSecret if self.step == AuthStep::Credentials => {
                Some(&mut self.form.client_secret)
            }
            AuthFocus::RedirectUri if self.step == AuthStep::Credentials => {
                Some(&mut self.form.redirect_uri)
            }
            AuthFocus::CallbackInput if self.step == AuthStep::ManualCallback => {
                Some(&mut self.callback_input)
            }
            _ => None,
        }
    }

    fn paste_clipboard(&mut self) {
        match read_clipboard_text() {
            Ok(text) => {
                self.paste_text(&text);
            }
            Err(error) => {
                self.set_error(format!(
                    "Could not read clipboard contents: {error}. Try your terminal paste shortcut too."
                ));
            }
        }
    }

    fn next_focus(&self) -> AuthFocus {
        let order = self.focus_order();
        let index = order
            .iter()
            .position(|focus| *focus == self.focus)
            .unwrap_or(0);
        order[(index + 1) % order.len()]
    }

    fn previous_focus(&self) -> AuthFocus {
        let order = self.focus_order();
        let index = order
            .iter()
            .position(|focus| *focus == self.focus)
            .unwrap_or(0);
        order[(index + order.len() - 1) % order.len()]
    }

    fn focus_order(&self) -> &'static [AuthFocus] {
        match self.step {
            AuthStep::CheckingSession => &[AuthFocus::OpenAppsPage],
            AuthStep::Credentials => &[
                AuthFocus::OpenAppsPage,
                AuthFocus::ClientId,
                AuthFocus::ClientSecret,
                AuthFocus::RedirectUri,
                AuthFocus::SaveAndContinue,
            ],
            AuthStep::WaitingForBrowser => &[
                AuthFocus::OpenBrowser,
                AuthFocus::PasteCallback,
                AuthFocus::BackToCredentials,
            ],
            AuthStep::ManualCallback => &[
                AuthFocus::CallbackInput,
                AuthFocus::SubmitCallback,
                AuthFocus::BackToBrowser,
            ],
        }
    }
}

impl CredentialsForm {
    pub fn from(credentials: Credentials) -> Self {
        Self {
            client_id: TextInput::new(credentials.client_id),
            client_secret: TextInput::new(credentials.client_secret),
            redirect_uri: TextInput::new(credentials.redirect_uri),
        }
    }
}

impl TextInput {
    pub fn new(value: String) -> Self {
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn display_value(&self, masked: bool) -> String {
        if masked {
            "*".repeat(self.value.chars().count())
        } else {
            self.value.clone()
        }
    }

    pub fn insert(&mut self, ch: char) {
        let mut chars = self.value.chars().collect::<Vec<_>>();
        chars.insert(self.cursor, ch);
        self.value = chars.into_iter().collect();
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, value: &str) {
        for ch in value.chars() {
            self.insert(ch);
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let mut chars = self.value.chars().collect::<Vec<_>>();
        chars.remove(self.cursor - 1);
        self.value = chars.into_iter().collect();
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        let mut chars = self.value.chars().collect::<Vec<_>>();
        if self.cursor >= chars.len() {
            return;
        }

        chars.remove(self.cursor);
        self.value = chars.into_iter().collect();
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor.min(self.value.chars().count());
    }
}

fn read_clipboard_text() -> Result<String, String> {
    if let Ok(mut clipboard) = Clipboard::new() {
        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                return Ok(text);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        for (program, args) in [
            ("wl-paste", vec!["--no-newline"]),
            ("xclip", vec!["-selection", "clipboard", "-o"]),
            ("xsel", vec!["--clipboard", "--output"]),
        ] {
            if let Ok(output) = Command::new(program).args(args).output()
                && output.status.success()
            {
                let text = String::from_utf8_lossy(&output.stdout).to_string();
                if !text.trim().is_empty() {
                    return Ok(text);
                }
            }
        }
    }

    Err("clipboard text was unavailable from both the terminal and system clipboard".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_supports_insert_and_backspace() {
        let mut input = TextInput::new("abc".to_string());
        input.move_left();
        input.insert('X');
        input.backspace();

        assert_eq!(input.value, "abc");
        assert_eq!(input.cursor, 2);
    }

    #[test]
    fn auth_state_switches_to_manual_callback() {
        let mut auth = AuthState::new(Credentials::default());
        auth.show_manual_callback("paste callback");

        assert_eq!(auth.step, AuthStep::ManualCallback);
        assert_eq!(auth.focus, AuthFocus::CallbackInput);
    }

    #[test]
    fn text_input_supports_pasting_strings() {
        let mut input = TextInput::new("abcd".to_string());
        input.move_left();
        input.move_left();
        input.insert_str("XYZ");

        assert_eq!(input.value, "abXYZcd");
        assert_eq!(input.cursor, 5);
    }
}
