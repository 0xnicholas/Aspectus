//! Aspectus HTTP client.
//!
//! Rust client library for Pandaria ecosystem projects to call
//! Aspectus's `/introspect` endpoint and management APIs.
//!
//! v0.1.0: Stub only. HTTP calls return `unimplemented!()`.
//! v0.2.0: Full reqwest-based implementation.

use aspectus_core::introspect::IntrospectResponse;
use serde::{Deserialize, Serialize};

/// Request body for `POST /introspect`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectRequest {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type_hint: Option<String>,
}

/// Client for interacting with an Aspectus server.
///
/// Holds the base URL and a Service Token for authenticating requests.
///
/// v0.2.0: Full reqwest-based HTTP implementation.
#[derive(Debug, Clone)]
pub struct AspectusClient {
    base_url: String,
    service_token: String,
    // client: reqwest::Client  (v0.2.0+)
}

impl AspectusClient {
    /// Create a new client targeting the given Aspectus server.
    pub fn new(base_url: impl Into<String>, service_token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            service_token: service_token.into(),
        }
    }

    /// Call `POST /introspect` to validate a token.
    ///
    /// v0.2.0: Sends `Authorization: Bearer {service_token}` header
    /// with `application/x-www-form-urlencoded` body.
    pub async fn introspect(&self, _token: &str) -> Result<IntrospectResponse, IntrospectError> {
        unimplemented!("v0.2.0: POST /introspect")
    }
}

/// Errors that can occur when calling the Aspectus API.
#[derive(Debug, thiserror::Error)]
pub enum IntrospectError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Service token rejected (401)")]
    Unauthorized,

    #[error("Unexpected response status: {0}")]
    UnexpectedStatus(u16),
}
