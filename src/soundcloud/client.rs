use anyhow::{Result, anyhow};
use reqwest::{Client, Method, StatusCode};
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub const API_BASE_URL: &str = "https://api.soundcloud.com";

#[derive(Debug, Clone)]
pub struct SoundcloudClient {
    http: Client,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub permalink_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MeResponse {
    username: String,
    permalink_url: Option<String>,
}

impl SoundcloudClient {
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()?;

        Ok(Self { http })
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub async fn get<T>(
        &self,
        path: &str,
        access_token: &str,
        query: &[(&str, String)],
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .http
            .request(Method::GET, format!("{API_BASE_URL}{path}"))
            .bearer_auth(access_token)
            .header("accept", "application/json; charset=utf-8")
            .query(query);

        self.send_json(request).await
    }

    pub async fn get_by_href<T>(&self, href: &str, access_token: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let request = self
            .http
            .request(Method::GET, sanitize_next_href(href)?)
            .bearer_auth(access_token)
            .header("accept", "application/json; charset=utf-8");

        self.send_json(request).await
    }

    pub async fn me(&self, access_token: &str) -> Result<AuthenticatedUser> {
        let me: MeResponse = self.get("/me", access_token, &[]).await?;
        Ok(AuthenticatedUser {
            username: me.username,
            permalink_url: me.permalink_url,
        })
    }

    pub async fn post_empty(&self, path: &str, access_token: &str) -> Result<()> {
        let request = self
            .http
            .request(Method::POST, format!("{API_BASE_URL}{path}"))
            .bearer_auth(access_token)
            .header("accept", "application/json; charset=utf-8");

        self.send_empty(request).await
    }

    pub async fn put_json<T>(&self, path: &str, access_token: &str, body: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let request = self
            .http
            .request(Method::PUT, format!("{API_BASE_URL}{path}"))
            .bearer_auth(access_token)
            .header("accept", "application/json; charset=utf-8")
            .header("content-type", "application/json; charset=utf-8")
            .json(body);

        self.send_empty(request).await
    }

    pub async fn put_form(
        &self,
        path: &str,
        access_token: &str,
        fields: &[(String, String)],
    ) -> Result<()> {
        let request = self
            .http
            .request(Method::PUT, format!("{API_BASE_URL}{path}"))
            .bearer_auth(access_token)
            .header("accept", "application/json; charset=utf-8")
            .form(fields);

        self.send_empty(request).await
    }

    async fn send_json<T>(&self, request: reqwest::RequestBuilder) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = request.send().await?;

        if response.status() != StatusCode::OK {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("SoundCloud request failed with {status}: {body}"));
        }

        Ok(response.json().await?)
    }

    async fn send_empty(&self, request: reqwest::RequestBuilder) -> Result<()> {
        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("SoundCloud request failed with {status}: {body}"));
        }

        Ok(())
    }
}

fn sanitize_next_href(href: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(href)?;

    if url.scheme() != "https" {
        url.set_scheme("https")
            .map_err(|()| anyhow!("could not update next_href scheme"))?;
    }

    if url.domain() != Some("api.soundcloud.com") {
        url.set_host(Some("api.soundcloud.com"))?;
    }

    Ok(url.to_string())
}
