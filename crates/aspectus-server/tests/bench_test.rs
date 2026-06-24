//! Performance benchmarks for /introspect path.
//!
//! Run with: DATABASE_URL=... REDIS_URL=... cargo bench -p aspectus-server
//!
//! For quick timing without criterion, use the test below:
//! DATABASE_URL=... REDIS_URL=... cargo test -p aspectus-server --test bench_test -- --nocapture

use std::sync::Arc;
use std::time::Instant;

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache};
use aspectus_core::project::Project;
use aspectus_core::store::{ServiceAccountStore, TenantStore};
use aspectus_server::db::{PgApiKeyStore, PgServiceAccountStore, PgTenantStore};

#[tokio::test]
async fn introspect_cold_path_bench() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());

    let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
    let redis_client = redis::Client::open(redis_url.as_str()).unwrap();
    let cache = RedisCache::new(redis_client)
        .await
        .expect("Redis connection failed");

    let tenant_store = PgTenantStore::new(pool.clone());
    let sa_store = PgServiceAccountStore::new(pool.clone());
    let api_key_store = Arc::new(PgApiKeyStore::new(pool));

    let creator = ApiKeyCreator::new(api_key_store.clone());
    let verifier = ApiKeyVerifier::new(api_key_store, cache);

    let tenant = tenant_store.create("bench-tenant").await.unwrap();
    let sa = sa_store.create(&tenant.id, "bench", None).await.unwrap();
    let key = creator
        .create(&tenant.id, &sa.id, Project::Pandaria, vec![], None)
        .await
        .unwrap();

    // Warm up
    verifier.verify(&key.key).await;

    // Measure
    let n = 100;
    let start = Instant::now();
    for _ in 0..n {
        verifier.verify(&key.key).await;
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / n as u128;
    println!("introspect x{n}: total={elapsed:?}, avg={avg_us}µs",);
    assert!(avg_us < 5000, "avg {avg_us}µs exceeds 5ms target");
}
