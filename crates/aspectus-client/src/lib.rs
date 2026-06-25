//! Aspectus HTTP client.
//!
//! Rust client library for Pandaria ecosystem projects to call
//! Aspectus's `/introspect` endpoint and management APIs.
//!
//! ## Local JWT verification (v0.9.0)
//!
//! For JWT tokens, prefer `verify_jwt()` over `introspect()` — it verifies
//! the RS256 signature locally using the JWKS public key, with zero network
//! overhead. Only falls back to `/introspect` for opaque tokens and API keys.
//!
//! ## Usage
//!
//! ```ignore
//! use aspectus_client::AspectusClient;
//!
//! let client = AspectusClient::new("http://localhost:3100", "my-service-token");
//!
//! // Local JWT verification (no network)
//! let response = client.verify_jwt("eyJ...").await?;
//! assert!(response.active);
//!
//! // Fallback: call /introspect for opaque / API Key
//! let response = client.introspect("pk_live_xxx").await?;
//! assert!(response.active);
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use aspectus_core::identity::IdentityType;
use aspectus_core::introspect::IntrospectResponse;
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// JWT claims as signed by Aspectus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub tenant_id: String,
    pub scope: String,
    #[serde(rename = "client_id")]
    pub client_id: String,
    #[serde(rename = "identity_type")]
    pub identity_type: String,
    pub aud: String,
    pub iss: String,
    pub iat: usize,
    pub exp: usize,
    pub jti: String,
}

/// Request body for `POST /introspect`.
#[derive(Debug, Clone, Serialize)]
pub struct IntrospectRequest {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type_hint: Option<String>,
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

    #[error("Failed to fetch JWKS: {0}")]
    JwksFetch(String),

    #[error("Invalid JWKS: no keys found")]
    JwksInvalid,

    #[error("JWT verification failed: {0}")]
    JwtVerify(String),
}

/// Client for interacting with an Aspectus server.
#[derive(Debug, Clone)]
pub struct AspectusClient {
    base_url: String,
    service_token: String,
    client: reqwest::Client,
    /// Cached JWT verifier for local validation.
    jwt_verifier: Option<JwtVerifier>,
}

impl AspectusClient {
    /// Create a new client targeting the given Aspectus server.
    pub fn new(base_url: impl Into<String>, service_token: impl Into<String>) -> Self {
        let base_url = base_url.into();
        let jwt_verifier = Some(JwtVerifier::new(&base_url));
        Self {
            base_url,
            service_token: service_token.into(),
            client: reqwest::Client::new(),
            jwt_verifier,
        }
    }

    /// Create a client with a pre-configured reqwest client.
    pub fn with_reqwest(
        base_url: impl Into<String>,
        service_token: impl Into<String>,
        client: reqwest::Client,
    ) -> Self {
        let base_url = base_url.into();
        let jwt_verifier = Some(JwtVerifier::new(&base_url));
        Self {
            base_url,
            service_token: service_token.into(),
            client,
            jwt_verifier,
        }
    }

    /// Create a client without JWT verifier (for test/offline use).
    pub fn without_jwt(base_url: impl Into<String>, service_token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            service_token: service_token.into(),
            client: reqwest::Client::new(),
            jwt_verifier: None,
        }
    }

    // ---- /introspect ----

    /// Call `POST /introspect` to validate a token.
    ///
    /// Returns the introspection response. For invalid tokens, `active` will be `false`.
    ///
    /// Prefer `verify_jwt()` for JWT tokens — it's faster (local only).
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

    // ---- Local JWT verification ----

    /// Verify a JWT locally using JWKS public key.
    ///
    /// **Zero network overhead** for the actual verification — only fetches
    /// JWKS on first call or after the cached key expires (1 hour TTL).
    ///
    /// Returns `Err(ClientError::JwtVerify(...))` if the token is expired,
    /// has invalid signature, or is otherwise unverifiable.
    pub async fn verify_jwt(&self, token: &str) -> Result<IntrospectResponse, ClientError> {
        let verifier = self
            .jwt_verifier
            .as_ref()
            .ok_or_else(|| ClientError::JwtVerify("JWT verifier disabled".into()))?;
        verifier.verify(token).await
    }

    /// Force-refresh the cached JWKS (e.g. after key rotation).
    pub async fn refresh_jwks(&self) -> Result<(), ClientError> {
        if let Some(verifier) = &self.jwt_verifier {
            verifier.fetch_jwks().await?;
        }
        Ok(())
    }

    /// Smart verification: JWT locally, everything else via /introspect.
    ///
    /// This is the recommended entry point for most consumers.
    /// It inspects the token prefix to decide the verification path.
    pub async fn verify(&self, token: &str) -> Result<IntrospectResponse, ClientError> {
        if token.starts_with("eyJ") {
            // JWT — verify locally
            self.verify_jwt(token).await
        } else {
            // Opaque or API Key — call /introspect
            self.introspect(token).await
        }
    }
}

// ---- JwtVerifier (internal) ----

struct JwtVerifier {
    jwks_url: String,
    client: reqwest::Client,
    decoding_key: Arc<RwLock<Option<DecodingKey>>>,
    last_refresh: Arc<RwLock<Option<Instant>>>,
}

impl Clone for JwtVerifier {
    fn clone(&self) -> Self {
        Self {
            jwks_url: self.jwks_url.clone(),
            client: self.client.clone(),
            decoding_key: self.decoding_key.clone(),
            last_refresh: self.last_refresh.clone(),
        }
    }
}

impl std::fmt::Debug for JwtVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let has_key = {
            let guard = self.decoding_key.try_read();
            guard.map(|g| g.is_some()).unwrap_or(false)
        };
        f.debug_struct("JwtVerifier")
            .field("jwks_url", &self.jwks_url)
            .field("has_key", &has_key)
            .finish()
    }
}

/// JWKS response from `/.well-known/jwks.json`.
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    #[serde(default)]
    alg: Option<String>,
    #[serde(default)]
    n: String,
    #[serde(default)]
    e: String,
    /// Key ID — reserved for future key rotation support.
    #[serde(default)]
    #[allow(dead_code)]
    kid: Option<String>,
}

impl JwtVerifier {
    fn new(base_url: &str) -> Self {
        Self {
            jwks_url: format!("{base_url}/.well-known/jwks.json"),
            client: reqwest::Client::new(),
            decoding_key: Arc::new(RwLock::new(None)),
            last_refresh: Arc::new(RwLock::new(None)),
        }
    }

    /// Fetch JWKS from the server and cache the decoding key (1h TTL).
    async fn fetch_jwks(&self) -> Result<(), ClientError> {
        let resp = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| ClientError::JwksFetch(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ClientError::JwksFetch(format!(
                "HTTP {}",
                resp.status().as_u16()
            )));
        }

        let jwks: JwksResponse = resp
            .json()
            .await
            .map_err(|e| ClientError::JwksFetch(format!("Failed to parse JWKS: {e}")))?;

        let key = jwks
            .keys
            .into_iter()
            .find(|k| k.alg.as_deref() == Some("RS256"))
            .ok_or(ClientError::JwksInvalid)?;

        let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
            .map_err(|e| ClientError::JwtVerify(format!("Invalid JWK: {e}")))?;

        let mut key_guard = self.decoding_key.write().await;
        *key_guard = Some(decoding_key);
        let mut last_guard = self.last_refresh.write().await;
        *last_guard = Some(Instant::now());

        Ok(())
    }

    /// Ensure JWKS is fetched and still valid.
    async fn ensure_jwks(&self) -> Result<(), ClientError> {
        let needs_refresh = {
            let last = self.last_refresh.read().await;
            match *last {
                Some(ts) => ts.elapsed() > Duration::from_secs(3600), // 1h TTL
                None => true,
            }
        };

        if needs_refresh {
            let key_exists = self.decoding_key.read().await.is_some();
            if !key_exists || needs_refresh {
                self.fetch_jwks().await?;
            }
        }

        Ok(())
    }

    /// Verify a JWT token locally.
    async fn verify(&self, token: &str) -> Result<IntrospectResponse, ClientError> {
        self.ensure_jwks().await?;

        let key = {
            let guard = self.decoding_key.read().await;
            guard
                .as_ref()
                .cloned()
                .ok_or_else(|| ClientError::JwtVerify("No JWKS key available".into()))?
        };

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.validate_exp = true;
        // Relax audience validation — the JWT audience is the project name
        validation.validate_aud = false;

        let data = decode::<JwtClaims>(token, &key, &validation)
            .map_err(|e| ClientError::JwtVerify(format!("{e}")))?;

        let claims = data.claims;

        let identity_type = claims
            .identity_type
            .as_str()
            .try_into()
            .unwrap_or(IdentityType::User);

        Ok(IntrospectResponse::active(
            claims.tenant_id,
            claims.sub,
            identity_type,
            claims.client_id,
            claims.scope,
            claims.exp as i64,
            None,
            "jwt",
        ))
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

    #[test]
    fn client_without_jwt() {
        let client = AspectusClient::without_jwt("http://localhost:3100", "test");
        assert!(client.jwt_verifier.is_none());
    }

    #[test]
    fn jwks_response_deserialize() {
        let json = r#"{"keys":[{"kty":"RSA","use":"sig","alg":"RS256","kid":"abc","n":"oV7","e":"AQAB"}]}"#;
        let jwks: JwksResponse = serde_json::from_str(json).unwrap();
        assert_eq!(jwks.keys.len(), 1);
        assert_eq!(jwks.keys[0].n, "oV7");
        assert_eq!(jwks.keys[0].e, "AQAB");
    }

    #[test]
    fn jwt_claims_deserialize() {
        let json = r#"{"sub":"user1","tenant_id":"t1","scope":"p:read","client_id":"pandaria","identity_type":"user","aud":"pandaria","iss":"https://aspectus","iat":1,"exp":9999999999,"jti":"abc"}"#;
        let claims: JwtClaims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.sub, "user1");
        assert_eq!(claims.identity_type, "user");
    }

    #[tokio::test]
    async fn verify_jwt_disabled_returns_error() {
        let client = AspectusClient::without_jwt("http://localhost:3100", "test");
        let err = client
            .verify_jwt("eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiIxIn0")
            .await
            .unwrap_err();
        assert!(matches!(err, ClientError::JwtVerify(_)));
    }

    #[tokio::test]
    async fn refresh_jwks_for_disabled_client_is_noop() {
        let client = AspectusClient::without_jwt("http://localhost:3100", "test");
        let result = client.refresh_jwks().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn fetch_jwks_errors_for_unavailable_server() {
        let client = AspectusClient::new("http://localhost:1", "test");
        let err = client.refresh_jwks().await.unwrap_err();
        assert!(
            matches!(err, ClientError::JwksFetch(_)),
            "expected JwksFetch error, got {err}"
        );
    }
}
