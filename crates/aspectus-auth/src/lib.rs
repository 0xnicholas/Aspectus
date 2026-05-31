//! Aspectus authentication logic.
//!
//! v0.2.0: Full implementation of API key creation/verification,
//! Service Token verification, and Redis caching.

mod cache;
pub mod jwt;
mod token_verifier;

use std::sync::Arc;

use chrono::Utc;
use sha2::{Digest, Sha256};

use aspectus_core::{
    api_key::{ApiKey, CreatedApiKey},
    error::CoreError,
    identity::IdentityType,
    introspect::IntrospectResponse,
    project::Project,
    store::{ApiKeyStore, ServiceTokenStore},
};

pub use cache::RedisCache;
pub use token_verifier::TokenVerifier;

// ---- Helpers ----

/// Extract raw bytes from a "pk_live_{hex}" formatted key.
fn extract_raw_from_key(token: &str) -> Option<Vec<u8>> {
    let encoded = token.strip_prefix("pk_live_")?;
    hex::decode(encoded).ok()
}

/// SHA256 hash as hex string.
fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Build an IntrospectResponse from a valid ApiKey row.
fn build_response(api_key: &ApiKey) -> IntrospectResponse {
    IntrospectResponse {
        active: true,
        tenant_id: Some(api_key.tenant_id.clone()),
        user_id: Some(api_key.service_account_id.clone()),
        identity_type: Some(IdentityType::ServiceAccount),
        client_id: Some(api_key.project.to_string()),
        scope: Some(api_key.scopes.join(" ")),
        token_type: Some("Bearer".into()),
        exp: api_key.expires_at.map(|dt| dt.timestamp()),
        quotas: None,
        token_format: Some("api_key".into()),
    }
}

/// Redis cache TTL: min(remaining_seconds / 10, 300).
fn compute_cache_ttl(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> u64 {
    match expires_at {
        Some(exp) => {
            let remaining = (exp - Utc::now()).num_seconds().max(0) as u64;
            (remaining / 10).min(300)
        }
        None => 300,
    }
}


/// Generate a random 21-char hex ID. Returns empty string on RNG failure.
pub(crate) fn generate_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map(|_| hex::encode(&bytes)[..21].to_string())
        .unwrap_or_default()
}

// ---- ApiKeyCreator ----

pub struct ApiKeyCreator {
    store: Arc<dyn ApiKeyStore>,
}

impl ApiKeyCreator {
    pub fn new(store: Arc<dyn ApiKeyStore>) -> Self {
        Self { store }
    }

    pub async fn create(
        &self,
        tenant_id: &str,
        service_account_id: &str,
        project: Project,
        scopes: Vec<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<CreatedApiKey, CoreError> {
        let id = generate_id();

        let mut raw = [0u8; 32];
        getrandom::getrandom(&mut raw).map_err(|e| CoreError::Internal(format!("RNG: {e}")))?;

        let key = format!("pk_live_{}", hex::encode(&raw));
        let key_hash = sha256_hex(&raw);
        let key_prefix = key[..17].to_string(); // "pk_live_" + 8 hex chars

        let _db_key = self
            .store
            .insert(
                &id, tenant_id, service_account_id, project, &key_hash, &key_prefix,
                &scopes, expires_at,
            )
            .await?;

        Ok(CreatedApiKey {
            id,
            key,
            key_prefix,
            project,
            scopes,
            expires_at,
        })
    }
}

// ---- ApiKeyVerifier ----

pub struct ApiKeyVerifier {
    store: Arc<dyn ApiKeyStore>,
    cache: RedisCache,
}

impl ApiKeyVerifier {
    pub fn new(store: Arc<dyn ApiKeyStore>, cache: RedisCache) -> Self {
        Self { store, cache }
    }

    /// Invalidate the cache entry for a specific key hash.
    /// Called when an API key is revoked.
    pub async fn invalidate_cache(&self, key_hash: &str) {
        self.cache.del(&format!("introspect:{key_hash}")).await;
    }

    /// Health check for the Redis cache.
    pub async fn cache_health(&self) -> Result<(), String> {
        self.cache.ping().await
    }

    pub async fn verify(&self, token: &str) -> IntrospectResponse {
        let raw = match extract_raw_from_key(token) {
            Some(r) => r,
            None => return IntrospectResponse::inactive(),
        };

        let key_hash = sha256_hex(&raw);
        let cache_key = format!("introspect:{key_hash}");

        // Redis cache lookup
        if let Some(cached) = self.cache.get_json::<IntrospectResponse>(&cache_key).await {
            if let Some(exp) = cached.exp {
                if exp < Utc::now().timestamp() {
                    return IntrospectResponse::inactive();
                }
            }
            return cached;
        }

        // PostgreSQL fallback
        match self.store.find_by_hash(&key_hash).await {
            Ok(Some(api_key)) => {
                if api_key.revoked_at.is_some() {
                    return IntrospectResponse::inactive();
                }
                if let Some(exp) = api_key.expires_at {
                    if exp < Utc::now() {
                        return IntrospectResponse::inactive();
                    }
                }

                let response = build_response(&api_key);
                let ttl = compute_cache_ttl(api_key.expires_at);
                self.cache.set_json(&cache_key, &response, ttl).await;
                response
            }
            Ok(None) => IntrospectResponse::inactive(),
            Err(_) => IntrospectResponse::inactive(),
        }
    }
}

// ---- ServiceTokenVerifier ----

pub struct ServiceTokenVerifier {
    store: Arc<dyn ServiceTokenStore>,
    cache: RedisCache,
}

impl ServiceTokenVerifier {
    pub fn new(store: Arc<dyn ServiceTokenStore>, cache: RedisCache) -> Self {
        Self { store, cache }
    }

    pub async fn verify(&self, token: &str) -> Option<Project> {
        let token_hash = sha256_hex(token.as_bytes());
        let cache_key = format!("svc_token:{token_hash}");

        // Redis (TTL=60s)
        if let Some(project_str) = self.cache.get(&cache_key).await {
            return project_str.parse().ok();
        }

        // PostgreSQL
        match self.store.find_by_hash(&token_hash).await {
            Ok(Some(project)) => {
                self.cache
                    .set(&cache_key, &project.to_string(), 60)
                    .await;
                Some(project)
            }
            _ => None,
        }
    }
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_raw_from_valid_key() {
        let raw = [0xabu8; 32];
        let key = format!("pk_live_{}", hex::encode(&raw));
        let extracted = extract_raw_from_key(&key).unwrap();
        assert_eq!(extracted, raw.to_vec());
    }

    #[test]
    fn extract_raw_from_invalid_prefix() {
        assert!(extract_raw_from_key("invalid").is_none());
        assert!(extract_raw_from_key("pk_live_zzz").is_none()); // invalid hex
    }

    #[test]
    fn sha256_is_deterministic() {
        let h1 = sha256_hex(b"hello");
        let h2 = sha256_hex(b"hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn compute_cache_ttl_no_expiry() {
        assert_eq!(compute_cache_ttl(None), 300);
    }

    #[test]
    fn compute_cache_ttl_with_expiry() {
        let future = Utc::now() + chrono::Duration::seconds(1000);
        let ttl = compute_cache_ttl(Some(future));
        assert!(ttl >= 99 && ttl <= 100, "expected ~100, got {ttl}");
    }

    #[test]
    fn compute_cache_ttl_capped() {
        let far_future = Utc::now() + chrono::Duration::seconds(10000);
        assert_eq!(compute_cache_ttl(Some(far_future)), 300); // capped
    }
}
