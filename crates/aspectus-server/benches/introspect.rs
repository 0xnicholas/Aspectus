//! Criterion benchmark for the `/introspect` hot path.
//!
//! Measures the core token verification path that underlies every consumer
//! request. Run with:
//!
//! ```bash
//! DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5432/aspectus \
//! REDIS_URL=redis://localhost:6379 \
//! cargo bench -p aspectus-server --bench introspect
//! ```
//!
//! The benchmark requires a running PostgreSQL + Redis because it exercises
//! the real verifier, cache, and stores.

use std::sync::Arc;

use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache, TokenVerifier};
use aspectus_core::project::Project;
use aspectus_core::store::{ServiceAccountStore, TenantStore};
use aspectus_server::db::{PgApiKeyStore, PgServiceAccountStore, PgTenantStore};
use criterion::{Criterion, criterion_group, criterion_main};

async fn setup() -> (Arc<TokenVerifier>, Arc<ApiKeyVerifier>, String, String) {
    let db_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set for the introspect benchmark");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());

    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    let redis_client =
        redis::Client::open(redis_url.as_str()).expect("Failed to create Redis client");

    let auth_cache = RedisCache::new(redis_client.clone())
        .await
        .expect("Failed to create auth cache");
    let jwt_cache = RedisCache::new(redis_client.clone())
        .await
        .expect("Failed to create JWT cache");

    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));
    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), auth_cache));

    let jwt_signer = Arc::new(
        JwtSigner::from_env().expect("JWT_PRIVATE_KEY_PEM / JWT_PUBLIC_KEY_PEM must be set"),
    );
    let jwt_verifier =
        Arc::new(JwtVerifier::from_env(jwt_cache).expect("JWT_PUBLIC_KEY_PEM must be set"));

    let token_verifier = Arc::new(TokenVerifier::new(
        api_key_verifier.clone(),
        jwt_verifier.clone(),
    ));

    // Create a tenant + service account + API key for the benchmark.
    let tenant_store = PgTenantStore::new(pool.clone());
    let sa_store = PgServiceAccountStore::new(pool.clone());

    let now = criterion::black_box(chrono::Utc::now().timestamp_millis());
    let tenant = tenant_store
        .create(&format!("bench-tenant-{now}"))
        .await
        .expect("Failed to create tenant");
    let sa = sa_store
        .create(&tenant.id, &format!("bench-sa-{now}"), None)
        .await
        .expect("Failed to create service account");
    let api_key = api_key_creator
        .create(
            &tenant.id,
            &sa.id,
            Project::Pandaria,
            vec!["pandaria:session:create".into()],
            None,
        )
        .await
        .expect("Failed to create API key");

    // Create a JWT signed for the same tenant/SA.
    let jwt = jwt_signer
        .sign_with_tenant_name(aspectus_auth::jwt::JwtSignRequest {
            sub: sa.id.clone(),
            tenant_id: tenant.id.clone(),
            tenant_name: None,
            project: Project::Pandaria,
            scopes: "pandaria:session:create".into(),
            identity_type: aspectus_core::identity::IdentityType::ServiceAccount,
            ttl_seconds: 900,
        })
        .expect("Failed to sign JWT");

    (token_verifier, api_key_verifier, api_key.key, jwt)
}

fn introspect_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let (token_verifier, api_key_verifier, api_key, jwt) = rt.block_on(setup());

    let mut group = c.benchmark_group("introspect");
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(100);

    // Cold path: API key verifier must hit Redis + PostgreSQL once.
    let api_key_verifier_cold = api_key_verifier.clone();
    let api_key_cold = api_key.clone();
    group.bench_function("api_key_cold", |b| {
        b.to_async(&rt).iter(|| async {
            criterion::black_box(api_key_verifier_cold.verify(&api_key_cold).await)
        })
    });

    // Hot path: TokenVerifier dispatches to the API-key verifier; the second
    // invocation should be served entirely from Redis.
    let token_verifier_hot = token_verifier.clone();
    let api_key_hot = api_key.clone();
    group.bench_function("api_key_hot", |b| {
        b.to_async(&rt)
            .iter(|| async { criterion::black_box(token_verifier_hot.verify(&api_key_hot).await) })
    });

    // JWT path: local verification with Redis revocation check.
    let token_verifier_jwt = token_verifier.clone();
    let jwt_bench = jwt.clone();
    group.bench_function("jwt", |b| {
        b.to_async(&rt)
            .iter(|| async { criterion::black_box(token_verifier_jwt.verify(&jwt_bench).await) })
    });

    group.finish();
}

criterion_group!(benches, introspect_benchmark);
criterion_main!(benches);
