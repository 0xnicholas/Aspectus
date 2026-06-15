use async_trait::async_trait;

use crate::{
    api_key::{ApiKey, ApiKeyListItem},
    audit_log::AuditLog,
    project::Project,
    service_account::ServiceAccount,
    tenant::Tenant,
};

/// Persistence layer for Tenant operations.
#[async_trait]
pub trait TenantStore: Send + Sync {
    async fn create(&self, name: &str) -> Result<Tenant, crate::error::CoreError>;
    async fn get_by_id(&self, id: &str) -> Result<Option<Tenant>, crate::error::CoreError>;
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

    async fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ServiceAccount>, crate::error::CoreError>;

    async fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<ServiceAccount>, crate::error::CoreError>;
}

/// Persistence layer for ApiKey operations.
#[async_trait]
pub trait ApiKeyStore: Send + Sync {
    /// Insert a new API key row. `key_hash` is already computed by the caller.
    async fn insert(
        &self,
        id: &str,
        tenant_id: &str,
        service_account_id: &str,
        project: Project,
        key_hash: &str,
        key_prefix: &str,
        scopes: &[String],
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<ApiKey, crate::error::CoreError>;

    /// Look up by sha256 hash. Returns the full ApiKey row.
    async fn find_by_hash(
        &self,
        key_hash: &str,
    ) -> Result<Option<ApiKey>, crate::error::CoreError>;

    /// Look up by primary key ID.
    async fn find_by_id(
        &self,
        id: &str,
    ) -> Result<Option<ApiKey>, crate::error::CoreError>;

    /// List all keys for a given service account (metadata only, no hash).
    async fn list_by_service_account(
        &self,
        service_account_id: &str,
    ) -> Result<Vec<ApiKeyListItem>, crate::error::CoreError>;

    /// Mark a key as revoked. Returns true if a row was actually revoked.
    async fn revoke(&self, id: &str) -> Result<bool, crate::error::CoreError>;
}

/// Persistence layer for AuditLog. Append-only: no UPDATE or DELETE.
#[async_trait]
pub trait AuditLogStore: Send + Sync {
    async fn append(&self, entry: AuditLog) -> Result<(), crate::error::CoreError>;
}

/// Persistence layer for ServiceToken lookup.
#[async_trait]
pub trait ServiceTokenStore: Send + Sync {
    async fn find_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Project>, crate::error::CoreError>;
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

    async fn get_by_id(&self, id: &str) -> Result<Option<crate::user::User>, crate::error::CoreError>;

    async fn list_by_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<crate::user::User>, crate::error::CoreError>;

    async fn set_suspended(&self, id: &str, suspended: bool) -> Result<bool, crate::error::CoreError>;
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
    async fn create(
        &self,
        client_id: &str,
        name: &str,
        redirect_uris: &[String],
    ) -> Result<(), crate::error::CoreError>;

    async fn list(&self) -> Result<Vec<(String, String, Vec<String>)>, crate::error::CoreError>;
    // Returns Vec<(client_id, name, redirect_uris)>

    async fn validate_redirect_uri(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<bool, crate::error::CoreError>;
}
