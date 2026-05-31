//! Aspectus authentication logic.
//!
//! Handles API key creation/verification, Service Token verification,
//! password hashing (argon2id), and JWT signing.
//!
//! v0.1.0: Stub only. All methods return `unimplemented!()`.
//! v0.2.0: Full implementation.

use aspectus_core::{api_key::CreatedApiKey, introspect::IntrospectResponse};

/// Verifies an API Key (by reference or opaque token).
///
/// Lookup path: sha256(token) → Redis cache → PostgreSQL fallback.
/// Returns `IntrospectResponse` with `active: true/false`.
///
/// v0.2.0: Full implementation with Redis + PostgreSQL.
pub struct ApiKeyVerifier;

impl ApiKeyVerifier {
    #[allow(unused)]
    pub fn new(/* db: PgPool, redis: RedisConnectionManager */) -> Self {
        Self
    }

    /// Verify a token and return an introspection response.
    pub async fn verify(&self, _token: &str) -> IntrospectResponse {
        unimplemented!("v0.2.0: sha256 → Redis → PostgreSQL")
    }
}

/// Verifies a Service Token used to authenticate the caller of `/introspect`.
///
/// Each Project has exactly one Service Token.
/// Lookup path: sha256(token) → Redis cache (TTL=60s) → PostgreSQL fallback.
///
/// v0.2.0: Full implementation.
pub struct ServiceTokenVerifier;

impl ServiceTokenVerifier {
    #[allow(unused)]
    pub fn new(/* db: PgPool, redis: RedisConnectionManager */) -> Self {
        Self
    }

    /// Verify a service token and return the associated Project identity.
    pub async fn verify(&self, _token: &str) -> Option<aspectus_core::project::Project> {
        unimplemented!("v0.2.0: sha256 → Redis → PostgreSQL")
    }
}

/// Creates API Keys and stores their sha256 hash.
///
/// The raw key is generated once and returned to the caller.
/// Only the sha256 hash is persisted.
///
/// v0.2.0: Full implementation.
pub struct ApiKeyCreator;

impl ApiKeyCreator {
    #[allow(unused)]
    pub fn new(/* db: PgPool */) -> Self {
        Self
    }

    /// Generate a new API Key.
    ///
    /// Returns the raw key (once). The caller is responsible for storing it safely.
    pub async fn create(
        &self,
        _tenant_id: &str,
        _service_account_id: &str,
        _project: aspectus_core::project::Project,
        _scopes: Vec<String>,
    ) -> Result<CreatedApiKey, aspectus_core::error::CoreError> {
        unimplemented!("v0.2.0: generate → sha256 → store → return raw")
    }
}
