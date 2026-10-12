//! GitHub sign-in via the OAuth device flow (RFC 8628) and token storage in the OS keychain.
//!
//! The token never leaves this process: the webview only sees the short user code it has to
//! show, and later the signed-in user's public profile.

use std::time::{Duration, Instant};

use reqwest::Client;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Public client id of the "Wishlist Editor" OAuth App; device flow needs no secret.
pub const CLIENT_ID: &str = "Ov23liPUzGIoKXZAKhsJ";
/// Enough to clone, push to and create public repositories, and to enable Pages on them.
const SCOPE: &str = "public_repo";
const KEYRING_SERVICE: &str = "dev.swchck.wishlist-editor";
const KEYRING_USER: &str = "github";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    interval: u64,
}

/// What the webview shows while the user confirms the code on github.com.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

impl From<&DeviceCode> for DevicePrompt {
    fn from(c: &DeviceCode) -> Self {
        Self {
            user_code: c.user_code.clone(),
            verification_uri: c.verification_uri.clone(),
            expires_in: c.expires_in,
        }
    }
}

pub async fn request_code(http: &Client) -> AppResult<DeviceCode> {
    let resp = http
        .post("https://github.com/login/device/code")
        .header(ACCEPT, "application/json")
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AppError::GitHub(format!(
            "device code request failed: {}",
            resp.status()
        )));
    }
    Ok(resp.json().await?)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

/// Polls until the user approves the code, then returns the access token.
pub async fn wait_for_token(
    http: &Client,
    code: &DeviceCode,
    cancelled: impl Fn() -> bool,
) -> AppResult<String> {
    // RFC 8628 §3.5: never poll faster than the server's interval, default 5 s
    let mut interval = code.interval.max(5);
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if cancelled() {
            return Err(AppError::AuthCancelled);
        }
        if Instant::now() >= deadline {
            return Err(AppError::AuthExpired);
        }
        let r: TokenResponse = http
            .post("https://github.com/login/oauth/access_token")
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", CLIENT_ID),
                ("device_code", code.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json()
            .await?;
        if let Some(token) = r.access_token {
            return Ok(token);
        }
        match r.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval = r.interval.unwrap_or(interval + 5),
            Some("expired_token") => return Err(AppError::AuthExpired),
            Some("access_denied") => return Err(AppError::AuthDenied),
            Some(other) => return Err(AppError::GitHub(other.to_owned())),
            None => return Err(AppError::GitHub("empty token response".into())),
        }
    }
}

fn entry() -> AppResult<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)?)
}

pub fn load_token() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn save_token(token: &str) -> AppResult<()> {
    Ok(entry()?.set_password(token)?)
}

pub fn clear_token() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
