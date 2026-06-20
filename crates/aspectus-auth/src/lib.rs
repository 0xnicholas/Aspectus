//! Aspectus authentication logic.
//!
//! v0.2.0: Full implementation of API key creation/verification,
//! Service Token verification, and Redis caching.
//! v0.4.0: JWT signing/verification + Opaque Token support.

mod cache;
pub mod password;
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

fn extract_raw_from_key(token: &str) -> Option<Vec<u8>> {
    let encoded = token
        .strip_prefix("pk_live_")
        .or_else(|| token.strip_prefix("ot_"))?;
    hex::decode(encoded).ok()
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

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

fn compute_cache_ttl(expires_at: Option<chrono::DateTime<chrono::Utc>>) -> u64 {
    match expires_at {
        Some(exp) => {
            let remaining = (exp - Utc::now()).num_seconds().max(0) as u64;
            (remaining / 10).min(300)
        }
        None => 300,
    }
}

pub(crate) fn generate_id() -> String {
    aspectus_core::generate_id()
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
        let key = format!("pk_live_{}", hex::encode(raw));
        let key_hash = sha256_hex(&raw);
        let key_prefix = key[..17].to_string();
        self.store.insert(aspectus_core::store::InsertApiKeyParams {
            id: id.clone(), tenant_id: tenant_id.to_string(), service_account_id: service_account_id.to_string(),
            project, key_hash, key_prefix: key_prefix.clone(),
            scopes: scopes.clone(), expires_at,
        }).await?;
        Ok(CreatedApiKey { id, key, key_prefix, project, scopes, expires_at })
    }

    /// Create an Opaque Token (v0.4.0). Uses ot_ prefix.
    pub async fn create_opaque(
        &self,
        tenant_id: &str,
        service_account_id: &str,
        project: Project,
        scopes: &str,
        ttl_seconds: u64,
    ) -> Result<CreatedApiKey, CoreError> {
        let id = generate_id();
        let mut raw = [0u8; 32];
        getrandom::getrandom(&mut raw).map_err(|e| CoreError::Internal(format!("RNG: {e}")))?;
        let key = format!("ot_{}", hex::encode(raw));
        let key_hash = sha256_hex(&raw);
        let key_prefix = key[..10].to_string();
        let expires_at = Some(Utc::now() + chrono::Duration::seconds(ttl_seconds as i64));
        let scopes_vec: Vec<String> = scopes.split_whitespace().map(String::from).collect();
        self.store.insert(aspectus_core::store::InsertApiKeyParams {
            id: id.clone(), tenant_id: tenant_id.to_string(), service_account_id: service_account_id.to_string(),
            project, key_hash, key_prefix: key_prefix.clone(),
            scopes: scopes_vec.clone(), expires_at,
        }).await?;
        Ok(CreatedApiKey { id, key, key_prefix, project, scopes: scopes_vec, expires_at })
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

    pub async fn invalidate_cache(&self, key_hash: &str) {
        self.cache.del(&format!("introspect:{key_hash}")).await;
    }

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
        if let Some(cached) = self.cache.get_json::<IntrospectResponse>(&cache_key).await {
            if let Some(exp) = cached.exp
                && exp < Utc::now().timestamp() { return IntrospectResponse::inactive(); }
            return cached;
        }
        match self.store.find_by_hash(&key_hash).await {
            Ok(Some(api_key)) => {
                if api_key.revoked_at.is_some() { return IntrospectResponse::inactive(); }
                if let Some(exp) = api_key.expires_at
                    && exp < Utc::now() { return IntrospectResponse::inactive(); }
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
        if let Some(project_str) = self.cache.get(&cache_key).await {
            return project_str.parse().ok();
        }
        match self.store.find_by_hash(&token_hash).await {
            Ok(Some(project)) => {
                self.cache.set(&cache_key, &project.to_string(), 60).await;
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
    fn extract_raw_from_opaque_key() {
        let raw = [0xabu8; 32];
        let key = format!("ot_{}", hex::encode(&raw));
        let extracted = extract_raw_from_key(&key).unwrap();
        assert_eq!(extracted, raw.to_vec());
    }

    #[test]
    fn extract_raw_from_invalid_prefix() {
        assert!(extract_raw_from_key("invalid").is_none());
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
        assert_eq!(compute_cache_ttl(Some(far_future)), 300);
    }

    #[test]
    fn build_response_sets_api_key_format() {
        let key = ApiKey {
            id: "k1".into(), tenant_id: "t1".into(), service_account_id: "sa1".into(),
            project: Project::Pandaria, key_hash: "h".into(), key_prefix: "p".into(),
            scopes: vec!["pandaria:session:create".into()],
            expires_at: None, revoked_at: None, created_at: Utc::now(),
        };
        let resp = build_response(&key);
        assert!(resp.active);
        assert_eq!(resp.token_format.as_deref(), Some("api_key"));
        assert_eq!(resp.identity_type, Some(IdentityType::ServiceAccount));
        assert_eq!(resp.client_id.as_deref(), Some("pandaria"));
        assert!(resp.scope.as_deref().unwrap().contains("pandaria:session:create"));
    }

    #[test]
    fn build_response_with_expiry() {
        let exp = Utc::now() + chrono::Duration::hours(24);
        let key = ApiKey {
            id: "k2".into(), tenant_id: "t2".into(), service_account_id: "sa2".into(),
            project: Project::Tavern, key_hash: "h2".into(), key_prefix: "p2".into(),
            scopes: vec![], expires_at: Some(exp), revoked_at: None, created_at: Utc::now(),
        };
        let resp = build_response(&key);
        assert!(resp.active);
        assert!(resp.exp.is_some());
        assert_eq!(resp.token_format.as_deref(), Some("api_key"));
    }

    #[test]
    fn generate_id_is_not_empty() {
        let id = generate_id();
        assert!(!id.is_empty());
        assert_eq!(id.len(), 27, "KSUID base62 is 27 characters");
    }

    #[test]
    fn generate_id_is_unique() {
        let ids: std::collections::HashSet<_> = (0..100).map(|_| generate_id()).collect();
        assert_eq!(ids.len(), 100, "100 generated IDs must all be unique");
    }

    #[test]
    fn inactive_response_has_no_fields() {
        let r = IntrospectResponse::inactive();
        assert!(!r.active);
        assert!(r.tenant_id.is_none());
        assert!(r.user_id.is_none());
        assert!(r.identity_type.is_none());
        assert!(r.client_id.is_none());
        assert!(r.scope.is_none());
        assert!(r.token_type.is_none());
        assert!(r.exp.is_none());
        assert!(r.quotas.is_none());
        assert!(r.token_format.is_none());
    }

    #[test]
    fn extract_raw_jwt_is_none() {
        // JWT tokens should NOT be processed by ApiKeyVerifier
        assert!(extract_raw_from_key("eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0").is_none());
    }

    #[test]
    fn extract_raw_empty_token() {
        assert!(extract_raw_from_key("").is_none());
    }

    #[test]
    fn sha256_hex_is_lowercase() {
        let result = sha256_hex(b"test");
        assert_eq!(result, result.to_lowercase());
        assert_eq!(result.len(), 64);
    }
}
