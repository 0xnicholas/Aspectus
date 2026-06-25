use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use aspectus_core::{
    error::CoreError, project::Project, service_token::ServiceToken, store::ServiceTokenStore,
};

pub struct PgServiceTokenStore {
    pool: PgPool,
}

impl PgServiceTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ServiceTokenStore for PgServiceTokenStore {
    async fn find_by_hash(&self, token_hash: &str) -> Result<Option<ServiceToken>, CoreError> {
        let row: Option<(
            String,
            String,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            "SELECT project, token_hash, token_prefix, created_at, updated_at, revoked_at \
             FROM service_tokens WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(row.and_then(
            |(project, token_hash, token_prefix, created_at, updated_at, revoked_at)| {
                project.parse().ok().map(|project| ServiceToken {
                    project,
                    token_hash,
                    token_prefix,
                    created_at,
                    updated_at,
                    revoked_at,
                })
            },
        ))
    }

    async fn list(&self) -> Result<Vec<ServiceToken>, CoreError> {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            "SELECT project, token_hash, token_prefix, created_at, updated_at, revoked_at \
                 FROM service_tokens ORDER BY project",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(project, token_hash, token_prefix, created_at, updated_at, revoked_at)| {
                    project.parse().ok().map(|project| ServiceToken {
                        project,
                        token_hash,
                        token_prefix,
                        created_at,
                        updated_at,
                        revoked_at,
                    })
                },
            )
            .collect())
    }

    async fn get_by_project(&self, project: &Project) -> Result<Option<ServiceToken>, CoreError> {
        let project_str = project.to_string();
        let row: Option<(
            String,
            String,
            Option<String>,
            DateTime<Utc>,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            "SELECT project, token_hash, token_prefix, created_at, updated_at, revoked_at \
                 FROM service_tokens WHERE project = $1",
        )
        .bind(&project_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(row.and_then(
            |(project, token_hash, token_prefix, created_at, updated_at, revoked_at)| {
                project.parse().ok().map(|project| ServiceToken {
                    project,
                    token_hash,
                    token_prefix,
                    created_at,
                    updated_at,
                    revoked_at,
                })
            },
        ))
    }

    async fn upsert(
        &self,
        project: Project,
        token_hash: &str,
        token_prefix: &str,
    ) -> Result<Option<String>, CoreError> {
        let project_str = project.to_string();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        let old: Option<(String,)> =
            sqlx::query_as("SELECT token_hash FROM service_tokens WHERE project = $1 FOR UPDATE")
                .bind(&project_str)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| CoreError::Internal(e.to_string()))?;

        sqlx::query(
            "INSERT INTO service_tokens (project, token_hash, token_prefix, created_at, updated_at) \
             VALUES ($1, $2, $3, now(), now()) \
             ON CONFLICT (project) DO UPDATE SET \
                 token_hash = EXCLUDED.token_hash, \
                 token_prefix = EXCLUDED.token_prefix, \
                 updated_at = now(), \
                 revoked_at = NULL",
        )
        .bind(&project_str)
        .bind(token_hash)
        .bind(token_prefix)
        .execute(&mut *tx)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(old.map(|(h,)| h))
    }

    async fn revoke(&self, project: &Project) -> Result<bool, CoreError> {
        let project_str = project.to_string();
        let result = sqlx::query(
            "UPDATE service_tokens SET revoked_at = now() \
             WHERE project = $1 AND revoked_at IS NULL",
        )
        .bind(&project_str)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }
}
