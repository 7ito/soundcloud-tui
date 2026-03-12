use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use log::{info, warn};
use rand::{RngCore, thread_rng};
use reqwest::StatusCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::{self, Instant},
};
use url::Url;

use crate::{
    config::{credentials::Credentials, tokens::TokenStore},
    soundcloud::client::{AuthenticatedUser, SoundcloudClient},
};

const AUTHORIZE_URL: &str = "https://secure.soundcloud.com/authorize";
const TOKEN_URL: &str = "https://secure.soundcloud.com/oauth/token";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);
const CALLBACK_READ_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub username: String,
    pub permalink_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthorizedSession {
    pub profile: AuthSession,
    pub credentials: Credentials,
    pub tokens: TokenStore,
}

#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    pub credentials: Credentials,
    pub verifier: String,
    pub state: String,
    pub authorize_url: String,
}

#[derive(Debug, Clone)]
pub struct AuthBootstrap {
    pub credentials: Credentials,
    pub tokens: Option<TokenStore>,
    pub warning: Option<String>,
}

#[derive(Debug)]
struct PkceBundle {
    verifier: String,
    challenge: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
    scope: Option<String>,
    token_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CallbackStatus {
    Ignored,
    Captured,
}

pub fn bootstrap() -> AuthBootstrap {
    let mut warning = None;

    let credentials = match Credentials::load_optional() {
        Ok(Some(credentials)) => credentials,
        Ok(None) => Credentials::default(),
        Err(error) => {
            warning = Some(error.to_string());
            Credentials::default()
        }
    };

    let tokens = match TokenStore::load() {
        Ok(tokens) => tokens,
        Err(error) => {
            warning = Some(match warning {
                Some(existing) => format!("{existing} {error}"),
                None => error.to_string(),
            });
            None
        }
    };

    let tokens = if tokens.is_some() && credentials.client_id.is_empty() {
        warning = Some(match warning {
            Some(existing) => format!(
                "{existing} Found saved SoundCloud session tokens in your OS keyring without matching app credentials. Sign in again."
            ),
            None => "Found saved SoundCloud session tokens in your OS keyring without matching app credentials. Sign in again.".to_string(),
        });
        None
    } else {
        tokens
    };

    AuthBootstrap {
        credentials,
        tokens,
        warning,
    }
}

pub async fn restore_saved_session(
    credentials: &Credentials,
    tokens: &TokenStore,
) -> Result<AuthorizedSession> {
    let client = SoundcloudClient::new()?;
    validate_or_refresh(&client, credentials, tokens).await
}

pub async fn ensure_fresh_tokens(
    credentials: &Credentials,
    tokens: &TokenStore,
) -> Result<TokenStore> {
    if !tokens.expires_soon() {
        return Ok(tokens.clone());
    }

    if !tokens.has_refresh_token() {
        bail!("Stored session expired and no refresh token is available.");
    }

    let client = SoundcloudClient::new()?;
    let refreshed = refresh_access_token(&client, credentials, tokens).await?;
    refreshed.save()?;
    Ok(refreshed)
}

pub fn prepare_authorization(credentials: Credentials) -> Result<AuthorizationRequest> {
    credentials.validate()?;
    let pkce = PkceBundle::generate();
    let authorize_url = build_authorize_url(&credentials, &pkce)?.to_string();

    Ok(AuthorizationRequest {
        credentials,
        verifier: pkce.verifier,
        state: pkce.state,
        authorize_url,
    })
}

pub async fn wait_for_callback(redirect_uri: &str, state: &str) -> Result<String> {
    let redirect_uri = Url::parse(redirect_uri)?;
    let host = redirect_uri
        .host_str()
        .context("redirect URI must include a host")?;
    let port = redirect_uri
        .port_or_known_default()
        .context("redirect URI must include an explicit or default port")?;

    if redirect_uri.scheme() != "http" || !matches!(host, "127.0.0.1" | "localhost") {
        bail!(
            "Automatic callback capture only supports http localhost redirect URIs; current redirect URI is {}",
            redirect_uri
        );
    }

    let listener = TcpListener::bind((host, port)).await.with_context(|| {
        format!(
            "Could not bind callback listener on {}:{}; check whether another process is using the port",
            host, port
        )
    })?;

    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    let expected_path = redirect_uri.path().to_string();
    let mut last_diagnostic = None;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let accept_timeout = remaining.min(CALLBACK_READ_TIMEOUT);
        let accept_result = time::timeout(accept_timeout, listener.accept()).await;
        let (mut stream, _) = match accept_result {
            Ok(Ok(stream)) => stream,
            Ok(Err(error)) => return Err(error).context("Failed to accept a localhost callback"),
            Err(_) => continue,
        };

        let mut buffer = [0_u8; 4096];
        let read_result = time::timeout(CALLBACK_READ_TIMEOUT, stream.read(&mut buffer)).await;
        let read = match read_result {
            Ok(Ok(read)) => read,
            Ok(Err(error)) => {
                return Err(error).context("Failed while reading a localhost callback request");
            }
            Err(_) => {
                last_diagnostic =
                    Some("Timed out while reading a localhost callback request".to_string());
                continue;
            }
        };

        if read == 0 {
            continue;
        }

        let request = String::from_utf8_lossy(&buffer[..read]);
        let first_line = request
            .lines()
            .next()
            .context("Received an empty callback request")?;
        let path = first_line
            .strip_prefix("GET ")
            .and_then(|line| line.split(" HTTP/").next())
            .context("Unexpected callback request format")?;

        let callback_url = format!("http://{}:{}{}", host, port, path);
        let (status, response_body, diagnostic) =
            classify_callback_request(&callback_url, &expected_path, state);

        if let Some(diagnostic) = diagnostic {
            warn!("ignored callback request: {diagnostic}");
            last_diagnostic = Some(diagnostic);
        }

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.shutdown().await;

        if status == CallbackStatus::Captured {
            return Ok(callback_url);
        }
    }

    let detail = last_diagnostic.unwrap_or_else(|| {
        "No valid callback containing both code and matching state was captured".to_string()
    });
    bail!("Timed out waiting for a valid browser callback. Last callback diagnostic: {detail}")
}

pub async fn complete_authorization(
    request: &AuthorizationRequest,
    callback_input: &str,
) -> Result<AuthorizedSession> {
    let code = extract_code_from_callback_input(callback_input, &request.state)?;
    let client = SoundcloudClient::new()?;
    let tokens = exchange_authorization_code(&client, request, &code).await?;
    tokens.save()?;

    let profile = client.me(&tokens.access_token).await?;
    Ok(AuthorizedSession {
        profile: AuthSession::from(profile),
        credentials: request.credentials.clone(),
        tokens,
    })
}

async fn validate_or_refresh(
    client: &SoundcloudClient,
    credentials: &Credentials,
    tokens: &TokenStore,
) -> Result<AuthorizedSession> {
    if !tokens.expires_soon() {
        match client.me(&tokens.access_token).await {
            Ok(profile) => {
                info!(
                    "reused existing SoundCloud session for {}",
                    profile.username
                );
                return Ok(AuthorizedSession {
                    profile: AuthSession::from(profile),
                    credentials: credentials.clone(),
                    tokens: tokens.clone(),
                });
            }
            Err(error) if error.to_string().contains("401") => {
                warn!("stored access token was rejected, attempting refresh");
            }
            Err(error) => return Err(error),
        }
    }

    if !tokens.has_refresh_token() {
        bail!("Stored session expired and no refresh token is available.");
    }

    let refreshed = refresh_access_token(client, credentials, tokens).await?;
    refreshed.save()?;

    let profile = client.me(&refreshed.access_token).await?;
    info!("refreshed SoundCloud session for {}", profile.username);
    Ok(AuthorizedSession {
        profile: AuthSession::from(profile),
        credentials: credentials.clone(),
        tokens: refreshed,
    })
}

fn extract_code_from_callback_input(input: &str, expected_state: &str) -> Result<String> {
    let callback_url = if let Ok(url) = Url::parse(input) {
        url
    } else {
        Url::parse(&format!(
            "http://localhost/callback?{}",
            input.trim_start_matches('?')
        ))
        .map_err(|error| anyhow!("Invalid callback URL: {error}"))?
    };

    let mut code = None;
    let mut state = None;
    let mut error = None;

    for (key, value) in callback_url.query_pairs() {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }

    if let Some(error) = error {
        bail!("SoundCloud authorization failed: {error}");
    }

    let returned_state = state.context("Callback response did not include `state`")?;
    if returned_state != expected_state {
        bail!(
            "Callback state mismatch; expected {}, got {}",
            expected_state,
            returned_state
        );
    }

    code.context("Callback response did not include `code`")
}

fn classify_callback_request(
    callback_url: &str,
    expected_path: &str,
    expected_state: &str,
) -> (CallbackStatus, &'static str, Option<String>) {
    let parsed = match Url::parse(callback_url) {
        Ok(url) => url,
        Err(error) => {
            return (
                CallbackStatus::Ignored,
                CALLBACK_WAITING_PAGE,
                Some(format!("Invalid callback URL {callback_url}: {error}")),
            );
        }
    };

    if parsed.path() != expected_path {
        return (
            CallbackStatus::Ignored,
            CALLBACK_WAITING_PAGE,
            Some(format!(
                "Ignoring unrelated localhost request for path {}",
                parsed.path()
            )),
        );
    }

    match extract_code_from_callback_input(callback_url, expected_state) {
        Ok(_) => (CallbackStatus::Captured, CALLBACK_SUCCESS_PAGE, None),
        Err(error) => (
            CallbackStatus::Ignored,
            CALLBACK_WAITING_PAGE,
            Some(format!("Ignoring callback URL {callback_url}: {error}")),
        ),
    }
}

async fn exchange_authorization_code(
    client: &SoundcloudClient,
    request: &AuthorizationRequest,
    code: &str,
) -> Result<TokenStore> {
    let response = client
        .http()
        .post(TOKEN_URL)
        .header("accept", "application/json; charset=utf-8")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", request.credentials.client_id.as_str()),
            ("client_secret", request.credentials.client_secret.as_str()),
            ("redirect_uri", request.credentials.redirect_uri.as_str()),
            ("code_verifier", request.verifier.as_str()),
            ("code", code),
        ])
        .send()
        .await?;

    parse_token_response(response, None).await
}

async fn refresh_access_token(
    client: &SoundcloudClient,
    credentials: &Credentials,
    existing_tokens: &TokenStore,
) -> Result<TokenStore> {
    let response = client
        .http()
        .post(TOKEN_URL)
        .header("accept", "application/json; charset=utf-8")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("refresh_token", existing_tokens.refresh_token.as_str()),
        ])
        .send()
        .await?;

    parse_token_response(response, Some(existing_tokens.refresh_token.as_str())).await
}

async fn parse_token_response(
    response: reqwest::Response,
    previous_refresh_token: Option<&str>,
) -> Result<TokenStore> {
    if response.status() != StatusCode::OK {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!("SoundCloud token exchange failed with {status}: {body}");
    }

    let token_response: TokenResponse = response.json().await?;
    let refresh_token = token_response
        .refresh_token
        .or_else(|| previous_refresh_token.map(str::to_string))
        .unwrap_or_default();

    Ok(TokenStore {
        access_token: token_response.access_token,
        refresh_token,
        token_type: token_response
            .token_type
            .unwrap_or_else(|| "Bearer".to_string()),
        scope: token_response.scope,
        expires_at_epoch: chrono::Utc::now().timestamp() + token_response.expires_in,
    })
}

fn build_authorize_url(credentials: &Credentials, pkce: &PkceBundle) -> Result<Url> {
    let mut url = Url::parse(AUTHORIZE_URL)?;
    url.query_pairs_mut()
        .append_pair("client_id", &credentials.client_id)
        .append_pair("redirect_uri", &credentials.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &pkce.state);
    Ok(url)
}

impl PkceBundle {
    fn generate() -> Self {
        let verifier = random_base64_url(48);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_base64_url(24);

        Self {
            verifier,
            challenge,
            state,
        }
    }
}

impl From<AuthenticatedUser> for AuthSession {
    fn from(user: AuthenticatedUser) -> Self {
        Self {
            username: user.username,
            permalink_url: user.permalink_url,
        }
    }
}

fn random_base64_url(bytes: usize) -> String {
    let mut random = vec![0_u8; bytes];
    thread_rng().fill_bytes(&mut random);
    URL_SAFE_NO_PAD.encode(random)
}

const CALLBACK_SUCCESS_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>soundcloud-tui</title>
<style>
:root {
    color-scheme: dark;
    --bg-top: #05384a;
    --bg-bottom: #021f29;
    --title: #f3efe5;
    --copy: #a8bcc3;
}

* {
    box-sizing: border-box;
}

body {
    margin: 0;
    min-height: 100vh;
    display: grid;
    place-items: center;
    background:
        radial-gradient(circle at top, rgba(69, 160, 188, 0.14), transparent 38%),
        linear-gradient(180deg, var(--bg-top), var(--bg-bottom));
    color: var(--title);
    font-family: "Avenir Next", "Segoe UI", "Helvetica Neue", sans-serif;
}

main {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding: 2rem;
    text-align: center;
}

h1 {
    margin: 0;
    font-size: clamp(3.5rem, 10vw, 6.75rem);
    font-weight: 600;
    letter-spacing: 0.08em;
}

p {
    margin: 0;
    color: var(--copy);
    font-family: "Iosevka Term", "Cascadia Mono", "SFMono-Regular", "Consolas", monospace;
    font-size: clamp(0.95rem, 1.4vw, 1.05rem);
}
</style>
</head>
<body>
<main>
<h1>soundcloud-tui</h1>
<p>You can close this tab and return to your terminal</p>
</main>
</body>
</html>"#;
const CALLBACK_FAILURE_PAGE: &str = "<html><body><h1>SoundCloud TUI authentication failed.</h1><p>Return to the terminal for the error details.</p></body></html>";
const CALLBACK_WAITING_PAGE: &str = "<html><body><h1>SoundCloud TUI is still waiting for a valid callback.</h1><p>You can return to the authorization tab or try the manual callback option in the terminal.</p></body></html>";

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::credentials::DEFAULT_REDIRECT_URI;
    use crate::config::secure_store::{
        CREDENTIALS_ENTRY, MemoryBackend, TOKENS_ENTRY, with_test_backend,
    };

    #[test]
    fn pkce_generation_produces_url_safe_values() {
        let pkce = PkceBundle::generate();

        assert!(pkce.verifier.len() >= 43);
        assert!(pkce.challenge.len() >= 43);
        assert!(pkce.state.len() >= 20);
        assert!(!pkce.verifier.contains('='));
    }

    #[test]
    fn callback_parser_rejects_bad_state() {
        let error = extract_code_from_callback_input(
            "http://127.0.0.1:8974/callback?code=test&state=wrong",
            "expected",
        )
        .unwrap_err();

        assert!(error.to_string().contains("state mismatch"));
    }

    #[test]
    fn callback_classifier_ignores_unrelated_requests() {
        let (status, _, diagnostic) =
            classify_callback_request("http://127.0.0.1:8974/favicon.ico", "/callback", "expected");

        assert_eq!(status, CallbackStatus::Ignored);
        assert!(diagnostic.unwrap().contains("unrelated localhost request"));
    }

    #[test]
    fn callback_classifier_captures_valid_callback() {
        let (status, _, diagnostic) = classify_callback_request(
            "http://127.0.0.1:8974/callback?code=test&state=expected",
            "/callback",
            "expected",
        );

        assert_eq!(status, CallbackStatus::Captured);
        assert!(diagnostic.is_none());
    }

    #[test]
    fn bootstrap_reads_credentials_and_tokens_from_os_keyring() {
        let credentials = Credentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: DEFAULT_REDIRECT_URI.to_string(),
        };
        let tokens = TokenStore {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_type: "Bearer".to_string(),
            scope: Some("non-expiring".to_string()),
            expires_at_epoch: chrono::Utc::now().timestamp() + 3600,
        };
        let backend = MemoryBackend::default()
            .with_entry(
                CREDENTIALS_ENTRY,
                &serde_json::to_string(&credentials).expect("serialize credentials"),
            )
            .with_entry(
                TOKENS_ENTRY,
                &serde_json::to_string(&tokens).expect("serialize tokens"),
            );

        with_test_backend(Arc::new(backend), || {
            let bootstrap = bootstrap();

            assert_eq!(bootstrap.credentials, credentials);
            assert_eq!(bootstrap.tokens, Some(tokens));
            assert!(bootstrap.warning.is_none());
        });
    }

    #[test]
    fn bootstrap_ignores_tokens_without_credentials() {
        let tokens = TokenStore {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_type: "Bearer".to_string(),
            scope: None,
            expires_at_epoch: chrono::Utc::now().timestamp() + 3600,
        };
        let backend = MemoryBackend::default().with_entry(
            TOKENS_ENTRY,
            &serde_json::to_string(&tokens).expect("serialize tokens"),
        );

        with_test_backend(Arc::new(backend), || {
            let bootstrap = bootstrap();

            assert_eq!(bootstrap.credentials, Credentials::default());
            assert!(bootstrap.tokens.is_none());
            assert!(
                bootstrap
                    .warning
                    .expect("warning")
                    .contains("without matching app credentials")
            );
        });
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn bootstrap_loads_linux_legacy_keyring_entries() {
        let credentials = Credentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: DEFAULT_REDIRECT_URI.to_string(),
        };
        let tokens = TokenStore {
            access_token: "access-token".to_string(),
            refresh_token: "refresh-token".to_string(),
            token_type: "Bearer".to_string(),
            scope: Some("legacy".to_string()),
            expires_at_epoch: chrono::Utc::now().timestamp() + 3600,
        };
        let backend = MemoryBackend::default()
            .with_legacy_entry(
                CREDENTIALS_ENTRY,
                &serde_json::to_string(&credentials).expect("serialize credentials"),
            )
            .with_legacy_entry(
                TOKENS_ENTRY,
                &serde_json::to_string(&tokens).expect("serialize tokens"),
            );

        with_test_backend(Arc::new(backend), || {
            let bootstrap = bootstrap();

            assert_eq!(bootstrap.credentials, credentials);
            assert_eq!(bootstrap.tokens, Some(tokens));
            assert!(bootstrap.warning.is_none());
        });
    }
}
