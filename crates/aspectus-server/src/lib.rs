//! Aspectus HTTP server.
//!
//! axum-based HTTP service providing the `/introspect` endpoint (v0.2.0+),
//! management APIs, and the `/health` endpoint.

pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod routes;

use std::sync::Arc;

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, ServiceTokenVerifier};
use db::{
    PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore,
};

/// Shared application state passed to all handlers via axum `State`.
#[derive(Clone)]
pub struct AppState {
    pub tenant_store: Arc<PgTenantStore>,
    pub service_account_store: Arc<PgServiceAccountStore>,
    pub api_key_store: Arc<PgApiKeyStore>,
    pub audit_log_store: Arc<PgAuditLogStore>,
    pub api_key_creator: Arc<ApiKeyCreator>,
    pub api_key_verifier: Arc<ApiKeyVerifier>,
    pub svc_token_verifier: Arc<ServiceTokenVerifier>,
    pub pool: sqlx::PgPool,
}
