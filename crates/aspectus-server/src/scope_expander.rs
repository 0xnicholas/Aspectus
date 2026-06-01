//! Scope expansion from user roles (v0.5.0).

use sqlx::PgPool;

/// Expands a user's assigned roles into a space-separated scope string.
pub struct ScopeExpander;

impl ScopeExpander {
    /// Given a user_id, return all scope names from their assigned roles.
    pub async fn expand(pool: &PgPool, user_id: &str) -> String {
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

        scopes.join(" ")
    }
}
