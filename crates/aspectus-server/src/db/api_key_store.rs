use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use aspectus_core::{
    api_key::{ApiKey, ApiKeyListItem},
    error::CoreError,
    project::Project,
    store::ApiKeyStore,
};

pub struct PgApiKeyStore {
    pool: PgPool,
}

impl PgApiKeyStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ApiKeyStore for PgApiKeyStore {
    async fn insert(
        &self,
        id: &str,
        tenant_id: &str,
        service_account_id: &str,
        project: Project,
        key_hash: &str,
        key_prefix: &str,
        scopes: &[String],
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ApiKey, CoreError> {
        sqlx::query_as::<_, ApiKey>(
            "INSERT INTO api_keys (id, tenant_id, service_account_id, project, \
             key_hash, key_prefix, scopes, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(service_account_id)
        .bind(project)
        .bind(key_hash)
        .bind(key_prefix)
        .bind(scopes)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn find_by_hash(&self, key_hash: &str) -> Result<Option<ApiKey>, CoreError> {
        sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE key_hash = $1")
            .bind(key_hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn list_by_service_account(
        &self,
        service_account_id: &str,
    ) -> Result<Vec<ApiKeyListItem>, CoreError> {
        sqlx::query_as::<_, ApiKeyListItem>(
            "SELECT id, service_account_id, project, key_prefix, scopes, \
             expires_at, revoked_at, created_at \
             FROM api_keys WHERE service_account_id = $1 ORDER BY created_at DESC",
        )
        .bind(service_account_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn revoke(&self, id: &str) -> Result<bool, CoreError> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}
