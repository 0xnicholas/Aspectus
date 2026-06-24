use async_trait::async_trait;
use sqlx::PgPool;

use crate::util::generate_id;
use aspectus_core::{
    error::CoreError, service_account::ServiceAccount, store::ServiceAccountStore,
};

pub struct PgServiceAccountStore {
    pool: PgPool,
}

impl PgServiceAccountStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ServiceAccountStore for PgServiceAccountStore {
    async fn create(
        &self,
        tenant_id: &str,
        label: &str,
        description: Option<&str>,
    ) -> Result<ServiceAccount, CoreError> {
        let id = generate_id();

        sqlx::query_as::<_, ServiceAccount>(
            "INSERT INTO service_accounts (id, tenant_id, label, description) \
             VALUES ($1, $2, $3, $4) RETURNING *",
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(label)
        .bind(description)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<ServiceAccount>, CoreError> {
        sqlx::query_as::<_, ServiceAccount>("SELECT * FROM service_accounts WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<ServiceAccount>, CoreError> {
        sqlx::query_as::<_, ServiceAccount>(
            "SELECT * FROM service_accounts WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }
}
