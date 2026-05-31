//! Integration tests for Aspectus v0.2.x.
//!
//! Tests the store/auth modules directly against the running PostgreSQL + Redis.
//! Set DATABASE_URL and REDIS_URL before running.
//!
//! ```bash
//! DATABASE_URL="postgresql://postgres:postgres@localhost:5432/aspectus" \
//! REDIS_URL="redis://:myredissecret@localhost:6379" \
//! cargo test -p aspectus-server --test integration_test -- --nocapture
//! ```

use std::sync::Arc;

use chrono::Utc;
use hex;
use sha2::{Digest, Sha256};

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier};
use aspectus_core::{
    audit_log::AuditLog,
    identity::IdentityType,
    project::Project,
    store::{ApiKeyStore, AuditLogStore, ServiceAccountStore, TenantStore},
};
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore,
};

fn svc_token_hash() -> String {
    hex::encode(Sha256::digest(b"aspectus-dev-pandaria-service-token"))
}

async fn setup() -> (
    PgTenantStore,
    PgServiceAccountStore,
    Arc<PgApiKeyStore>,
    PgAuditLogStore,
    ApiKeyCreator,
    ApiKeyVerifier,
    ServiceTokenVerifier,
) {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());

    let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
    let redis_client = redis::Client::open(redis_url.as_str()).unwrap();
    let cache = RedisCache::new(redis_client).await;

    // Seed service token if not exists
    let _ = sqlx::query(
        "INSERT INTO service_tokens (project, token_hash) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(Project::Pandaria)
    .bind(svc_token_hash())
    .execute(&pool)
    .await;

    let tenant_store = PgTenantStore::new(pool.clone());
    let sa_store = PgServiceAccountStore::new(pool.clone());
    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));
    let audit_store = PgAuditLogStore::new(pool.clone());

    let creator = ApiKeyCreator::new(api_key_store.clone());
    let verifier = ApiKeyVerifier::new(api_key_store.clone(), cache.clone());

    let svc_store = Arc::new(PgServiceTokenStore::new(pool));
    let svc_verifier = ServiceTokenVerifier::new(svc_store, cache);

    (tenant_store, sa_store, api_key_store, audit_store, creator, verifier, svc_verifier)
}

#[tokio::test]
async fn full_introspect_flow() {
    let (tenant_store, sa_store, api_key_store, _al, creator, verifier, svc_verifier) =
        setup().await;

    // Verify Service Token
    let project = svc_verifier
        .verify("aspectus-dev-pandaria-service-token")
        .await;
    assert_eq!(project, Some(Project::Pandaria));

    // Create tenant
    let tenant = tenant_store.create("integration-test-tenant").await.unwrap();
    assert!(!tenant.id.is_empty());

    // Create SA
    let sa = sa_store
        .create(&tenant.id, "integration-test-sa", None)
        .await
        .unwrap();
    assert_eq!(sa.tenant_id, tenant.id);

    // Create API Key
    let created = creator
        .create(
            &tenant.id,
            &sa.id,
            Project::Pandaria,
            vec!["pandaria:session:create".into()],
            None,
        )
        .await
        .unwrap();
    assert!(created.key.starts_with("pk_live_"));

    // Introspect — active
    let response = verifier.verify(&created.key).await;
    assert!(response.active);
    assert_eq!(response.tenant_id.as_deref(), Some(tenant.id.as_str()));
    assert_eq!(
        response.identity_type,
        Some(IdentityType::ServiceAccount)
    );
    assert_eq!(response.client_id.as_deref(), Some("pandaria"));
    assert!(response.scope.as_deref().unwrap().contains("pandaria:session:create"));

    // Revoke + invalidate cache
    let revoked = api_key_store.revoke(&created.id).await.unwrap();
    assert!(revoked);

    // Invalidate cache (in production this happens in the HTTP handler)
    let hash = hex::encode(Sha256::digest(hex::decode(created.key.strip_prefix("pk_live_").unwrap()).unwrap()));
    verifier.invalidate_cache(&hash).await;

    // Introspect — inactive
    let response = verifier.verify(&created.key).await;
    assert!(!response.active, "Expected inactive after revoke");
}

#[tokio::test]
async fn unknown_key_returns_inactive() {
    let (_ts, _ss, _as, _al, _cr, verifier, _sv) = setup().await;
    let response = verifier.verify("pk_live_nonexistentkey1234567890abcdef").await;
    assert!(!response.active);
}

#[tokio::test]
async fn malformed_key_returns_inactive() {
    let (_ts, _ss, _as, _al, _cr, verifier, _sv) = setup().await;
    let response = verifier.verify("not-a-valid-key").await;
    assert!(!response.active);
}

#[tokio::test]
async fn invalid_service_token_returns_none() {
    let (_ts, _ss, _as, _al, _cr, _sv, svc_verifier) = setup().await;
    let result = svc_verifier.verify("wrong-token").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn audit_log_appended() {
    let (tenant_store, _ss, _as, audit_store, _cr, _sv, _svc) = setup().await;
    let tenant = tenant_store.create("audit-tenant").await.unwrap();
    let entry = AuditLog {
        id: format!("audit-{:06}", (Utc::now().timestamp_millis() % 1_000_000).abs()),
        tenant_id: tenant.id,
        actor_id: "mgmt".into(),
        actor_type: IdentityType::ServiceAccount,
        action: "test.event".into(),
        target_type: "test".into(),
        target_id: "test-001".into(),
        metadata: serde_json::json!({"test": true}),
        created_at: chrono::Utc::now(),
    };
    audit_store.append(entry).await.unwrap();
}

#[tokio::test]
async fn cache_hit_on_repeat_introspect() {
    let (tenant_store, sa_store, _as, _al, creator, verifier, _sv) = setup().await;

    let tenant = tenant_store
        .create("cache-test-tenant")
        .await
        .unwrap();
    let sa = sa_store
        .create(&tenant.id, "cache-test-sa", None)
        .await
        .unwrap();
    let created = creator
        .create(&tenant.id, &sa.id, Project::Pandaria, vec![], None)
        .await
        .unwrap();

    let r1 = verifier.verify(&created.key).await;
    assert!(r1.active);
    let r2 = verifier.verify(&created.key).await;
    assert!(r2.active);
    assert_eq!(r2.tenant_id, r1.tenant_id);
}
