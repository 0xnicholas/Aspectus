//! Scope expansion from user roles (v0.5.0, cached v0.10.0).

use aspectus_auth::RedisCache;
use sqlx::PgPool;

/// Expands a user's assigned roles into a space-separated scope string.
pub struct ScopeExpander;

impl ScopeExpander {
    /// Given a user_id, return all scope names from their assigned roles.
    ///
    /// v0.10.0: Accepts an optional Redis cache. When provided, scope
    /// expansions are cached for 60 seconds to avoid repeated DB joins.
    pub async fn expand(
        pool: &PgPool,
        user_id: &str,
        cache: Option<&RedisCache>,
    ) -> String {
        let cache_key = format!("scope_expand:{user_id}");

        // Check cache first
        if let Some(cache) = cache {
            if let Some(cached) = cache.get(&cache_key).await {
                return cached;
            }
        }

        let scopes: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT s.name
             FROM users_roles ur
             JOIN roles_scopes rs ON rs.role_id = ur.role_id
             JOIN scopes s ON s.id = rs.scope_id
             WHERE ur.user_id = $1
             ORDER BY s.name",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let result = scopes.join(" ");

        // Cache for 60 seconds
        if let Some(cache) = cache {
            cache.set(&cache_key, &result, 60).await;
        }

        result
    }

    /// Invalidate the scope cache for a user (called on role assignment/removal).
    pub async fn invalidate(cache: &RedisCache, user_id: &str) {
        let cache_key = format!("scope_expand:{user_id}");
        cache.del(&cache_key).await;
    }
}
