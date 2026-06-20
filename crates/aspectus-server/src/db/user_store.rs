use async_trait::async_trait;
use sqlx::PgPool;

use aspectus_core::{error::CoreError, store::UserStore, user::User};

pub struct PgUserStore {
    pool: PgPool,
}

impl PgUserStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserStore for PgUserStore {
    async fn create(
        &self,
        tenant_id: &str,
        email: &str,
        password_hash: &str,
        display_name: Option<&str>,
    ) -> Result<User, CoreError> {
        let id = crate::util::generate_id();
        sqlx::query_as::<_, User>(
            "INSERT INTO users (id, tenant_id, email, password_hash, display_name) \
             VALUES ($1, $2, $3, $4, $5) RETURNING *",
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<User>, CoreError> {
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<User>, CoreError> {
        sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn set_suspended(&self, id: &str, suspended: bool) -> Result<bool, CoreError> {
        let result = sqlx::query("UPDATE users SET is_suspended = $1 WHERE id = $2")
            .bind(suspended)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}
