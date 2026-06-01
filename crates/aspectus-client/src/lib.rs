//! Aspectus HTTP client.
//!
//! Rust client library for Pandaria ecosystem projects to call
//! Aspectus's `/introspect` endpoint and management APIs.
//!
//! ## Usage
//!
//! ```ignore
//! use aspectus_client::AspectusClient;
//!
//! let client = AspectusClient::new("http://localhost:3100", "my-service-token");
//! let response = client.introspect("pk_live_xxx").await?;
//! assert!(response.active);
//! ```

use aspectus_core::introspect::IntrospectResponse;
use serde::Serialize;

/// Request body for `POST /introspect`.
#[derive(Debug, Clone, Serialize)]
pub struct IntrospectRequest {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type_hint: Option<String>,
}

/// Client for interacting with an Aspectus server.
#[derive(Debug, Clone)]
pub struct AspectusClient {
    base_url: String,
    service_token: String,
    client: reqwest::Client,
}

/// Errors that can occur when calling the Aspectus API.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Service token rejected (401)")]
    Unauthorized,

    #[error("Unexpected response status: {0}")]
    UnexpectedStatus(u16),

    #[error("Failed to parse response: {0}")]
    Parse(String),
}

impl AspectusClient {
    /// Create a new client targeting the given Aspectus server.
    pub fn new(base_url: impl Into<String>, service_token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            service_token: service_token.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Create a client with a pre-configured reqwest client.
    pub fn with_reqwest(
        base_url: impl Into<String>,
        service_token: impl Into<String>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            service_token: service_token.into(),
            client,
        }
    }

    /// Call `POST /introspect` to validate a token.
    ///
    /// Returns the introspection response. For invalid tokens, `active` will be `false`.
    pub async fn introspect(&self, token: &str) -> Result<IntrospectResponse, ClientError> {
        let resp = self
            .client
            .post(format!("{}/introspect", self.base_url))
            .header("Authorization", format!("Bearer {}", self.service_token))
            .form(&[("token", token)])
            .send()
            .await?;

        match resp.status().as_u16() {
            200 => resp
                .json::<IntrospectResponse>()
                .await
                .map_err(|e| ClientError::Parse(e.to_string())),
            401 => Err(ClientError::Unauthorized),
            other => Err(ClientError::UnexpectedStatus(other)),
        }
    }

    /// Call `POST /introspect` with a token type hint.
    pub async fn introspect_with_hint(
        &self,
        token: &str,
        hint: &str,
    ) -> Result<IntrospectResponse, ClientError> {
        let resp = self
            .client
            .post(format!("{}/introspect", self.base_url))
            .header("Authorization", format!("Bearer {}", self.service_token))
            .form(&[("token", token), ("token_type_hint", hint)])
            .send()
            .await?;

        match resp.status().as_u16() {
            200 => resp
                .json::<IntrospectResponse>()
                .await
                .map_err(|e| ClientError::Parse(e.to_string())),
            401 => Err(ClientError::Unauthorized),
            other => Err(ClientError::UnexpectedStatus(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructs() {
        let client = AspectusClient::new("http://localhost:3100", "test-token");
        assert_eq!(client.base_url, "http://localhost:3100");
        assert_eq!(client.service_token, "test-token");
    }
}
