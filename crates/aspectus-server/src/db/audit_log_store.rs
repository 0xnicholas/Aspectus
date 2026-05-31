use async_trait::async_trait;
use sqlx::PgPool;

use aspectus_core::{
    audit_log::AuditLog,
    error::CoreError,
    store::AuditLogStore,
};

pub struct PgAuditLogStore {
    pool: PgPool,
}

impl PgAuditLogStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditLogStore for PgAuditLogStore {
    async fn append(&self, entry: AuditLog) -> Result<(), CoreError> {
        sqlx::query(
            "INSERT INTO audit_logs (id, tenant_id, actor_id, actor_type, \
             action, target_type, target_id, metadata, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&entry.id)
        .bind(&entry.tenant_id)
        .bind(&entry.actor_id)
        .bind(entry.actor_type)
        .bind(&entry.action)
        .bind(&entry.target_type)
        .bind(&entry.target_id)
        .bind(&entry.metadata)
        .bind(entry.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(())
    }
}
