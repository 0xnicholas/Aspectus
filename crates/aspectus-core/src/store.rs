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
