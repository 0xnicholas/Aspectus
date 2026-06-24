//! Scope expansion from user roles (v0.5.0, cached v0.10.0).

use aspectus_auth::RedisCache;
use sqlx::PgPool;

/// Expands a user's assigned roles into a space-separated scope string.
pub struct ScopeExpander;

/// ADR-016: Derive the list of distinct `project` segments from a scope string.
/// Used by `issue_tokens` to populate `available_projects` in the response.
///
/// Rules:
/// - Scope format: `project:resource:action` (see ADR-006).
/// - A valid scope MUST contain at least one `:` separator.
/// - Take the first `:` segment as the project.
/// - Drop empty segments, the literal `*`, and malformed scopes (no `:`).
/// - Deduplicate and sort for stable output.
///
/// Returns a sorted `Vec<String>` of unique projects.
pub fn projects_from_scopes(scopes: &str) -> Vec<String> {
    scopes
        .split_whitespace()
        // Only keep scopes with at least one `:` (well-formed per ADR-006).
        .filter(|s| s.contains(':'))
        .filter_map(|s| s.split(':').next())
        .filter(|p| !p.is_empty() && *p != "*")
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(String::from)
        .collect()
}

impl ScopeExpander {
    /// Given a user_id, return all scope names from their assigned roles.
    ///
    /// v0.10.0: Accepts an optional Redis cache. When provided, scope
    /// expansions are cached for 60 seconds to avoid repeated DB joins.
    pub async fn expand(pool: &PgPool, user_id: &str, cache: Option<&RedisCache>) -> String {
        let cache_key = format!("scope_expand:{user_id}");

        // Check cache first
        if let Some(cache) = cache
            && let Some(cached) = cache.get(&cache_key).await
        {
            return cached;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_from_scopes_basic() {
        let scopes = "pandaria:session:read pandaria:agent:execute constell:agent:read";
        // BTreeSet gives alphabetical order, not insertion order
        assert_eq!(
            projects_from_scopes(scopes),
            vec!["constell".to_string(), "pandaria".to_string()]
        );
    }

    #[test]
    fn projects_from_scopes_dedup() {
        let scopes = "pandaria:session:read pandaria:agent:execute pandaria:session:create";
        assert_eq!(projects_from_scopes(scopes), vec!["pandaria".to_string()]);
    }

    #[test]
    fn projects_from_scopes_empty() {
        assert!(projects_from_scopes("").is_empty());
    }

    #[test]
    fn projects_from_scopes_no_colon() {
        // Malformed scope — drop it.
        assert!(projects_from_scopes("notascope").is_empty());
    }

    #[test]
    fn projects_from_scopes_skip_wildcard() {
        let scopes = "*:read pandaria:session:read";
        assert_eq!(projects_from_scopes(scopes), vec!["pandaria".to_string()]);
    }

    #[test]
    fn projects_from_scopes_multiple() {
        let scopes = "constell:agent:publish tokencamp:token:consume heirloom:resource:read";
        assert_eq!(
            projects_from_scopes(scopes),
            vec![
                "constell".to_string(),
                "heirloom".to_string(),
                "tokencamp".to_string(),
            ]
        );
    }

    #[test]
    fn projects_from_scopes_sorted_stable() {
        let scopes = "constell:agent:publish pandaria:session:read tokencamp:monthly:read";
        let result = projects_from_scopes(scopes);
        // BTreeSet gives alphabetical order, not insertion order
        assert_eq!(
            result,
            vec![
                "constell".to_string(),
                "pandaria".to_string(),
                "tokencamp".to_string(),
            ]
        );
    }
}
