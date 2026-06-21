//! Unified token verification — dispatches to the correct verifier by prefix.

use std::sync::Arc;

use aspectus_core::introspect::IntrospectResponse;

use crate::jwt::JwtVerifier;

use super::ApiKeyVerifier;

/// Dispatches token verification based on prefix.
///
/// - `pk_live_*` → API Key (ApiKeyVerifier)
/// - `eyJ*`      → JWT (JwtVerifier)
/// - `ot_*`      → Opaque (reuses ApiKeyVerifier path via extract_raw)
pub struct TokenVerifier {
    api_key: Arc<ApiKeyVerifier>,
    jwt: Arc<JwtVerifier>,
}

impl TokenVerifier {
    pub fn new(api_key: Arc<ApiKeyVerifier>, jwt: Arc<JwtVerifier>) -> Self {
        Self { api_key, jwt }
    }

    pub async fn verify(&self, token: &str) -> IntrospectResponse {
        if token.starts_with("eyJ") {
            self.jwt.verify(token).await
        } else {
            // API Key (pk_live_*) and Opaque (ot_*) both use sha256→Redis→PG
            // The ApiKeyVerifier extracts raw bytes from both prefixes
            let mut response = self.api_key.verify(token).await;
            // Override token_format for opaque tokens
            if response.active && token.starts_with("ot_") {
                response.token_format = Some("opaque".into());
            }
            response
        }
    }
}
