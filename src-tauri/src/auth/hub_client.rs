use serde::{Deserialize, Serialize};

use super::client::UserInfo;

pub struct ResolveResult {
    pub server_id: String,
    pub address: String,
    pub verified_domain: Option<String>,
}

/// Client for ss13hub session token authentication.
pub struct HubClient {
    base_url: String,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct LoginRequest {
    username_or_email: String,
    password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    totp_code: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub expire_time: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct Requires2FAResponse {
    requires_2fa: Option<bool>,
}

#[derive(Deserialize)]
struct OAuthExchangeResponse {
    token: Option<String>,
    expire_time: Option<String>,
    user_id: Option<String>,
    username: Option<String>,
    #[serde(default)]
    requires_2fa: bool,
}

impl HubClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    fn from_config() -> Result<Self, HubAuthError> {
        if let Ok(override_url) = std::env::var("SS13LAUNCHER_HUBAPI") {
            return Ok(Self::new(&override_url));
        }

        let config = crate::config::get_config();
        let base_url = config
            .urls
            .hub_api
            .ok_or_else(|| HubAuthError::Config("Hub API URL not configured".to_string()))?;
        Ok(Self::new(base_url))
    }

    /// Log in with username/email and password. Returns session token and user info.
    pub async fn login(
        username_or_email: &str,
        password: &str,
        totp_code: Option<&str>,
    ) -> Result<LoginResponse, HubAuthError> {
        let client = Self::from_config()?;

        let response = client
            .http
            .post(format!("{}/auth/login", client.base_url))
            .json(&LoginRequest {
                username_or_email: username_or_email.to_string(),
                password: password.to_string(),
                totp_code: totp_code.map(String::from),
            })
            .send()
            .await
            .map_err(|e| HubAuthError::Network(format!("Failed to connect: {e}")))?;

        let status = response.status();

        if status.is_success() {
            return response
                .json::<LoginResponse>()
                .await
                .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")));
        }

        let body = response.text().await.unwrap_or_default();

        // Check for 2FA requirement
        if status == reqwest::StatusCode::UNAUTHORIZED {
            if let Ok(r) = serde_json::from_str::<Requires2FAResponse>(&body) {
                if r.requires_2fa == Some(true) {
                    return Err(HubAuthError::Requires2FA);
                }
            }
        }

        let message = serde_json::from_str::<ErrorResponse>(&body)
            .map_or_else(|_| format!("HTTP {status}"), |e| e.error);

        match status {
            s if s == reqwest::StatusCode::UNAUTHORIZED => Err(HubAuthError::InvalidCredentials),
            s if s == reqwest::StatusCode::FORBIDDEN => Err(HubAuthError::AccountLocked),
            _ => Err(HubAuthError::Server(message)),
        }
    }

    /// Refresh a session token. Returns new token and expiry.
    pub async fn refresh(token: &str) -> Result<LoginResponse, HubAuthError> {
        let client = Self::from_config()?;

        let url = format!("{}/auth/refresh", client.base_url);
        tracing::info!("Hub refresh: POST {url}");
        let response = client
            .http
            .post(&url)
            .header("Authorization", format!("SS13Auth {token}"))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Hub refresh: network error: {e}");
                HubAuthError::Network(format!("Failed to connect: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Hub refresh: HTTP {status}, body={body}");
            return Err(HubAuthError::TokenExpired);
        }

        tracing::info!("Hub refresh: HTTP {status} OK");
        response
            .json::<LoginResponse>()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))
    }

    pub async fn join(token: &str, server_id: &str) -> Result<String, HubAuthError> {
        let client = Self::from_config()?;

        let url = format!("{}/session/join", client.base_url);
        tracing::info!(%url, server_id, "HubClient::join request");

        let response = client
            .http
            .post(&url)
            .header("Authorization", format!("SS13Auth {token}"))
            .json(&serde_json::json!({
                "server_id": server_id,
            }))
            .send()
            .await
            .map_err(|e| {
                tracing::error!(%url, %e, "HubClient::join network error");
                HubAuthError::Network(format!("Failed to connect: {e}"))
            })?;

        tracing::info!(%url, status = %response.status(), "HubClient::join response");

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(%url, %status, %body, "HubClient::join failed");
            return Err(HubAuthError::Server(format!(
                "Join failed (HTTP {status}): {body}"
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))?;

        body.get("nonce")
            .ok_or(HubAuthError::Server(
                "missing nonce in response".to_string(),
            ))?
            .as_str()
            .map(String::from)
            .ok_or_else(|| HubAuthError::Server("missing nonce in response".into()))
    }

    pub async fn join_complete(
        token: &str,
        nonce: &str,
        hwid_version: u32,
        components: &[serde_json::Value],
        signature: Option<&str>,
        variant: &str,
    ) -> Result<String, HubAuthError> {
        let client = Self::from_config()?;

        let url = format!("{}/session/join/complete", client.base_url);
        tracing::info!(%url, "HubClient::join_complete request");

        let response = client
            .http
            .post(&url)
            .header("Authorization", format!("SS13Auth {token}"))
            .json(&serde_json::json!({
                "nonce": nonce,
                "hwid_version": hwid_version,
                "components": components,
                "signature": signature,
                "variant": variant,
            }))
            .send()
            .await
            .map_err(|e| {
                tracing::error!(%url, %e, "HubClient::join_complete network error");
                HubAuthError::Network(format!("Failed to connect: {e}"))
            })?;

        tracing::info!(%url, status = %response.status(), "HubClient::join_complete response");

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(%url, %status, %body, "HubClient::join_complete failed");

            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                if err.error == "hwid_key_expired" {
                    return Err(HubAuthError::HwidKeyExpired);
                }
            }

            return Err(HubAuthError::Server(format!(
                "Join complete failed (HTTP {status}): {body}"
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))?;

        body["auth_ticket"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| HubAuthError::Server("missing auth_ticket in response".into()))
    }

    /// Resolve a host:port to a server UUID and trust metadata.
    pub async fn resolve_server(host: &str, port: u16) -> Result<ResolveResult, HubAuthError> {
        let client = Self::from_config()?;

        let response = client
            .http
            .get(format!(
                "{}/servers/resolve?host={}&port={}",
                client.base_url, host, port
            ))
            .send()
            .await
            .map_err(|e| HubAuthError::Network(format!("Failed to connect: {e}")))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(HubAuthError::NotFound);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HubAuthError::Server(format!(
                "Server resolve failed (HTTP {status}): {body}"
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))?;

        let server_id = body["server_id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| HubAuthError::Server("missing server_id in response".into()))?;

        let address = body["address"]
            .as_str()
            .map(String::from)
            .unwrap_or_default();

        let verified_domain = body["verified_domain"].as_str().map(String::from);

        Ok(ResolveResult {
            server_id,
            address,
            verified_domain,
        })
    }

    /// Exchange an OAuth login code for a session token.
    pub async fn oauth_exchange(
        code: &str,
        totp_code: Option<&str>,
        recovery_code: Option<&str>,
    ) -> Result<LoginResponse, HubAuthError> {
        let client = Self::from_config()?;

        let mut body = serde_json::json!({ "code": code });
        if let Some(totp) = totp_code {
            body["totp_code"] = serde_json::Value::String(totp.to_string());
        }
        if let Some(recovery) = recovery_code {
            body["recovery_code"] = serde_json::Value::String(recovery.to_string());
        }

        let response = client
            .http
            .post(format!("{}/auth/oauth/exchange", client.base_url))
            .json(&body)
            .send()
            .await
            .map_err(|e| HubAuthError::Network(format!("Failed to connect: {e}")))?;

        let status = response.status();

        if status.is_success() {
            let exchange: OAuthExchangeResponse = response
                .json()
                .await
                .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))?;

            if exchange.requires_2fa {
                return Err(HubAuthError::Requires2FA);
            }

            return Ok(LoginResponse {
                token: exchange.token.unwrap_or_default(),
                expire_time: exchange.expire_time.unwrap_or_default(),
                user_id: exchange.user_id.unwrap_or_default(),
                username: exchange.username.unwrap_or_default(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED {
            let body_text = response.text().await.unwrap_or_default();
            if let Ok(r) = serde_json::from_str::<Requires2FAResponse>(&body_text) {
                if r.requires_2fa == Some(true) {
                    return Err(HubAuthError::Requires2FA);
                }
            }
            return Err(HubAuthError::InvalidCredentials);
        }

        let body_text = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<ErrorResponse>(&body_text)
            .map_or_else(|_| "OAuth exchange failed".to_string(), |e| e.error);
        Err(HubAuthError::Server(message))
    }

    /// Fetch the hub's public config (e.g. available OAuth providers).
    pub async fn get_hub_config() -> Result<serde_json::Value, HubAuthError> {
        let client = Self::from_config()?;

        let response = client
            .http
            .get(format!("{}/config", client.base_url))
            .send()
            .await
            .map_err(|e| HubAuthError::Network(format!("Failed to connect: {e}")))?;

        response
            .json()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))
    }

    /// Fetch user profile using a session token.
    pub async fn get_profile(token: &str) -> Result<UserInfo, HubAuthError> {
        let client = Self::from_config()?;

        let url = format!("{}/account", client.base_url);
        tracing::info!("Hub get_profile: GET {url}");
        let response = client
            .http
            .get(&url)
            .header("Authorization", format!("SS13Auth {token}"))
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("Hub get_profile: network error: {e}");
                HubAuthError::Network(format!("Failed to connect: {e}"))
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Hub get_profile: HTTP {status}, body={body}");
            return Err(HubAuthError::TokenExpired);
        }

        tracing::info!("Hub get_profile: HTTP {status} OK");
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| HubAuthError::Network(format!("Invalid response: {e}")))?;

        let user = &body["user"];
        Ok(UserInfo {
            sub: user["id"].as_str().unwrap_or_default().to_string(),
            name: user["username"].as_str().map(String::from),
            preferred_username: user["username"].as_str().map(String::from),
            email: user["email"].as_str().map(String::from),
            email_verified: user["email_confirmed"].as_bool(),
        })
    }
}

#[derive(Debug)]
pub enum HubAuthError {
    InvalidCredentials,
    Requires2FA,
    AccountLocked,
    TokenExpired,
    HwidKeyExpired,
    NotFound,
    Network(String),
    Server(String),
    Config(String),
}

impl std::fmt::Display for HubAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCredentials => write!(f, "Invalid username or password"),
            Self::Requires2FA => write!(f, "2FA code required"),
            Self::AccountLocked => write!(f, "Account is locked"),
            Self::TokenExpired => write!(f, "Session expired, please log in again"),
            Self::HwidKeyExpired => write!(f, "Launcher update required"),
            Self::NotFound => write!(f, "Not found"),
            Self::Network(msg) => write!(f, "{msg}"),
            Self::Server(msg) => write!(f, "{msg}"),
            Self::Config(msg) => write!(f, "{msg}"),
        }
    }
}
