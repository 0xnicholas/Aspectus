use async_trait::async_trait;

use crate::{
    api_key::{ApiKey, ApiKeyListItem},
    audit_log::AuditLog,
    project::Project,
    service_account::ServiceAccount,
    service_token::ServiceToken,
    tenant::Tenant,
};

/// Persistence layer for Tenant operations.
#[async_trait]
pub trait TenantStore: Send + Sync {
    async fn create(&self, name: &str) -> Result<Tenant, crate::error::CoreError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Tenant>, crate::error::CoreError>;
    async fn list(&self) -> Result<Vec<Tenant>, crate::error::CoreError>;
}

/// Persistence layer for ServiceAccount operations.
#[async_trait]
pub trait ServiceAccountStore: Send + Sync {
    async fn create(
        &self,
        tenant_id: &str,
        label: &str,
        description: Option<&str>,
    ) -> Result<ServiceAccount, crate::error::CoreError>;

    async fn get_by_id(&self, id: &str) -> Result<Option<ServiceAccount>, crate::error::CoreError>;

    async fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<ServiceAccount>, crate::error::CoreError>;
}

/// Persistence layer for ApiKey operations.
///
/// Parameters for inserting a new API key.
pub struct InsertApiKeyParams {
    pub id: String,
    pub tenant_id: String,
    pub service_account_id: String,
    pub project: Project,
    pub key_hash: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    /// Insert a new API key row. `key_hash` is already computed by the caller.
    async fn insert(&self, params: InsertApiKeyParams) -> Result<ApiKey, crate::error::CoreError>;

    /// Look up by sha256 hash. Returns the full ApiKey row.
    async fn find_by_hash(&self, key_hash: &str)
    -> Result<Option<ApiKey>, crate::error::CoreError>;

    /// Look up by primary key ID.
    async fn find_by_id(&self, id: &str) -> Result<Option<ApiKey>, crate::error::CoreError>;

    /// List all keys for a given service account (metadata only, no hash).
    async fn list_by_service_account(
        &self,
        service_account_id: &str,
    ) -> Result<Vec<ApiKeyListItem>, crate::error::CoreError>;

    /// Mark a key as revoked. Returns true if a row was actually revoked.
    async fn revoke(&self, id: &str) -> Result<bool, crate::error::CoreError>;
}

/// Filter for querying audit logs.
#[derive(Debug, Default, serde::Deserialize)]
pub struct AuditLogFilter {
    pub tenant_id: Option<String>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub actor_id: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default = "default_audit_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_audit_limit() -> i64 {
    100
}

/// Persistence layer for AuditLog. Append-only: no UPDATE or DELETE.
#[async_trait]
pub trait AuditLogStore: Send + Sync {
    async fn append(&self, entry: AuditLog) -> Result<(), crate::error::CoreError>;

    /// Query audit logs with optional filters, ordered by `created_at DESC`.
    async fn list(&self, filter: AuditLogFilter) -> Result<Vec<AuditLog>, crate::error::CoreError>;
}

/// Persistence layer for ServiceToken lookup and lifecycle management.
#[async_trait]
pub trait ServiceTokenStore: Send + Sync {
    /// Look up a token by hash, returning the full row only if the token is active.
    ///
    /// Callers must verify the hash with a constant-time comparison and check
    /// [`ServiceToken::is_active`] themselves; the store only filters out rows
    /// whose `revoked_at` is set.
    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<crate::service_token::ServiceToken>, crate::error::CoreError>;

    /// List all stored service tokens, including revoked ones.
    async fn list(&self) -> Result<Vec<ServiceToken>, crate::error::CoreError>;

    /// Get a service token by project, including revoked ones.
    async fn get_by_project(
        &self,
        project: &Project,
    ) -> Result<Option<ServiceToken>, crate::error::CoreError>;

    /// Insert or replace the token for a project. Returns the previous hash
    /// (if any) so the caller can invalidate Redis caches.
    async fn upsert(
        &self,
        project: Project,
        token_hash: &str,
        token_prefix: &str,
    ) -> Result<Option<String>, crate::error::CoreError>;

    /// Soft-revoke a project's token. Returns true if a token was revoked.
    async fn revoke(&self, project: &Project) -> Result<bool, crate::error::CoreError>;
}

/// Persistence layer for User operations (v0.5.0).
#[async_trait]
pub trait UserStore: Send + Sync {
    async fn create(
        &self,
        tenant_id: &str,
        email: &str,
        password_hash: &str,
        display_name: Option<&str>,
    ) -> Result<crate::user::User, crate::error::CoreError>;

    async fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<crate::user::User>, crate::error::CoreError>;

    async fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<crate::user::User>, crate::error::CoreError>;

    async fn set_suspended(
        &self,
        id: &str,
        suspended: bool,
    ) -> Result<bool, crate::error::CoreError>;

    async fn set_password(
        &self,
        id: &str,
        password_hash: &str,
    ) -> Result<bool, crate::error::CoreError>;

    /// Atomically increment failed login attempts and, if the new count reaches
    /// `threshold`, set `locked_until` to `now + lockout_duration_secs`.
    /// Returns the new attempt count and current lockout time (if any).
    async fn record_failed_login(
        &self,
        id: &str,
        threshold: i32,
        lockout_duration_secs: i64,
    ) -> Result<(i32, Option<chrono::DateTime<chrono::Utc>>), crate::error::CoreError>;

    /// Clear failed login attempts and any active lockout. Called on successful
    /// login or when an administrator manually unlocks an account.
    async fn clear_failed_logins(&self, id: &str) -> Result<bool, crate::error::CoreError>;
}

/// Persistence layer for OAuth2 authorization codes (v0.9.0).
#[async_trait]
pub trait AuthorizationCodeStore: Send + Sync {
    async fn create_code(
        &self,
        code: &str,
        user_id: &str,
        client_id: &str,
        redirect_uri: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), crate::error::CoreError>;

    /// Atomically marks a code as used and returns its payload.
    /// Returns None if the code is invalid, expired, or already used.
    async fn exchange_code(
        &self,
        code: &str,
    ) -> Result<Option<(String, String, String)>, crate::error::CoreError>;
    // Returns (user_id, client_id, redirect_uri)
}

/// Persistence layer for OAuth2 refresh tokens (v0.9.0).
#[async_trait]
pub trait RefreshTokenStore: Send + Sync {
    async fn create(
        &self,
        token_hash: &str,
        user_id: &str,
        client_id: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), crate::error::CoreError>;

    /// Atomically revokes and returns the refresh token payload.
    /// Returns None if the token is invalid, expired, or already revoked.
    async fn rotate(
        &self,
        token_hash: &str,
    ) -> Result<Option<(String, String, String)>, crate::error::CoreError>;
    // Returns (user_id, client_id, old_hash)

    /// Revoke all active refresh tokens for a user (replay attack response).
    async fn revoke_all_for_user(&self, user_id: &str) -> Result<u64, crate::error::CoreError>;

    /// Look up a token by hash regardless of revocation status (for replay detection).
    async fn find_by_hash_any(
        &self,
        token_hash: &str,
    ) -> Result<Option<(String, bool)>, crate::error::CoreError>;
    // Returns (user_id, is_revoked)
}

/// Persistence layer for OAuth2 client registration (v0.9.0).
#[async_trait]
pub trait OAuth2ClientStore: Send + Sync {
    /// Create a new OAuth2 client. `client_secret_hash` is the SHA-256 of the
    /// plain-text secret, which the caller must generate and return exactly once.
    async fn create(
        &self,
        client_id: &str,
        name: &str,
        redirect_uris: &[String],
        client_secret_hash: &str,
    ) -> Result<(), crate::error::CoreError>;

    async fn list(&self) -> Result<Vec<(String, String, Vec<String>)>, crate::error::CoreError>;
    // Returns Vec<(client_id, name, redirect_uris)>

    async fn validate_redirect_uri(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<bool, crate::error::CoreError>;

    /// Validate a client_secret against the stored hash.
    /// Returns `Ok(true)` if the client has no secret configured (backward compat),
    /// or if the provided secret matches the hash.
    async fn validate_client_secret(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<bool, crate::error::CoreError>;
}
