use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use aspectus_core::error::CoreError;
use aspectus_core::store::{AuthorizationCodeStore, RefreshTokenStore, OAuth2ClientStore};

pub struct PgAuthorizationCodeStore {
    pool: PgPool,
}

pub struct PgRefreshTokenStore {
    pool: PgPool,
}

pub struct PgOAuth2ClientStore {
    pool: PgPool,
}

impl PgAuthorizationCodeStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgRefreshTokenStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PgOAuth2ClientStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthorizationCodeStore for PgAuthorizationCodeStore {
    async fn create_code(
        &self,
        code: &str,
        user_id: &str,
        client_id: &str,
        redirect_uri: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        sqlx::query(
            "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(code)
        .bind(user_id)
        .bind(client_id)
        .bind(redirect_uri)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn exchange_code(
        &self,
        code: &str,
    ) -> Result<Option<(String, String, String)>, CoreError> {
        sqlx::query_as::<_, (String, String, String)>(
            "UPDATE authorization_codes SET used = true \
             WHERE code = $1 AND used = false AND expires_at > now() \
             RETURNING user_id, client_id, redirect_uri",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }
}

#[async_trait]
impl RefreshTokenStore for PgRefreshTokenStore {
    async fn create(
        &self,
        token_hash: &str,
        user_id: &str,
        client_id: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        sqlx::query(
            "INSERT INTO refresh_tokens (token_hash, user_id, client_id, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(token_hash)
        .bind(user_id)
        .bind(client_id)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn rotate(
        &self,
        token_hash: &str,
    ) -> Result<Option<(String, String, String)>, CoreError> {
        sqlx::query_as::<_, (String, String, String)>(
            "UPDATE refresh_tokens SET revoked_at = now() \
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() \
             RETURNING user_id, client_id, token_hash",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn revoke_all_for_user(&self, user_id: &str) -> Result<u64, CoreError> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now() \
             WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn find_by_hash_any(
        &self,
        token_hash: &str,
    ) -> Result<Option<(String, bool)>, CoreError> {
        sqlx::query_as::<_, (String, bool)>(
            "SELECT user_id, revoked_at IS NOT NULL FROM refresh_tokens WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }
}

#[async_trait]
impl OAuth2ClientStore for PgOAuth2ClientStore {
    async fn create(
        &self,
        client_id: &str,
        name: &str,
        redirect_uris: &[String],
    ) -> Result<(), CoreError> {
        sqlx::query(
            "INSERT INTO oauth2_clients (client_id, name, redirect_uris) VALUES ($1, $2, $3)",
        )
        .bind(client_id)
        .bind(name)
        .bind(redirect_uris)
        .execute(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<(String, String, Vec<String>)>, CoreError> {
        sqlx::query_as::<_, (String, String, Vec<String>)>(
            "SELECT client_id, name, redirect_uris FROM oauth2_clients ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }

    async fn validate_redirect_uri(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Result<bool, CoreError> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM oauth2_clients \
             WHERE client_id = $1 AND $2 = ANY(redirect_uris))",
        )
        .bind(client_id)
        .bind(redirect_uri)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| CoreError::Internal(e.to_string()))
    }
}
