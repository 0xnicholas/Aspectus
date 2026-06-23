use async_trait::async_trait;
use sqlx::{PgPool, QueryBuilder};

use aspectus_core::{
    audit_log::AuditLog,
    error::CoreError,
    store::{AuditLogFilter, AuditLogStore},
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

    async fn list(&self, filter: AuditLogFilter) -> Result<Vec<AuditLog>, CoreError> {
        let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
            "SELECT id, tenant_id, actor_id, actor_type, action, target_type, target_id, metadata, created_at \
             FROM audit_logs WHERE 1=1",
        );

        if let Some(tenant_id) = filter.tenant_id {
            qb.push(" AND tenant_id = ").push_bind(tenant_id);
        }
        if let Some(action) = filter.action {
            qb.push(" AND action = ").push_bind(action);
        }
        if let Some(target_type) = filter.target_type {
            qb.push(" AND target_type = ").push_bind(target_type);
        }
        if let Some(target_id) = filter.target_id {
            qb.push(" AND target_id = ").push_bind(target_id);
        }
        if let Some(actor_id) = filter.actor_id {
            qb.push(" AND actor_id = ").push_bind(actor_id);
        }
        if let Some(from) = filter.from {
            qb.push(" AND created_at >= ").push_bind(from);
        }
        if let Some(to) = filter.to {
            qb.push(" AND created_at <= ").push_bind(to);
        }

        qb.push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(filter.limit)
            .push(" OFFSET ")
            .push_bind(filter.offset);

        qb.build_query_as::<AuditLog>()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }
}
