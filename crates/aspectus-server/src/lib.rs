//! Aspectus HTTP server.
//!
//! axum-based HTTP service providing the `/introspect` endpoint,
//! management APIs, and the `/health` endpoint.

pub mod config;
pub mod db;
pub mod scope_expander;
pub mod error;
pub mod middleware;
pub mod routes;
pub mod util;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, ServiceTokenVerifier, RedisCache, TokenVerifier};
use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use db::{PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgTenantStore, PgUserStore,
    PgAuthorizationCodeStore, PgRefreshTokenStore, PgOAuth2ClientStore};

/// Shared application state passed to all handlers via axum `State`.
#[derive(Clone)]
pub struct AppState {
    pub tenant_store: Arc<PgTenantStore>,
    pub service_account_store: Arc<PgServiceAccountStore>,
    pub api_key_store: Arc<PgApiKeyStore>,
    pub audit_log_store: Arc<PgAuditLogStore>,
    pub api_key_creator: Arc<ApiKeyCreator>,
    pub api_key_verifier: Arc<ApiKeyVerifier>,
    pub token_verifier: Arc<TokenVerifier>,
    pub svc_token_verifier: Arc<ServiceTokenVerifier>,
    pub jwt_signer: Arc<JwtSigner>,
    pub jwt_verifier: Arc<JwtVerifier>,
    pub user_store: Arc<PgUserStore>,
    pub auth_code_store: Arc<PgAuthorizationCodeStore>,
    pub refresh_token_store: Arc<PgRefreshTokenStore>,
    pub oauth_client_store: Arc<PgOAuth2ClientStore>,
    /// Redis cache for scope expansion (TTL=60s).
    pub scope_cache: Arc<RedisCache>,
    pub pool: sqlx::PgPool,
}
