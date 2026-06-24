//! Shared test harness for HTTP integration tests.
//!
//! Uses DATABASE_URL and REDIS_URL environment variables.
//! For a fully self-contained experience, start dependencies first:
//! ```bash
//! docker compose up -d
//! DATABASE_URL=... REDIS_URL=... cargo test -p aspectus-server --test http_tests
//! ```

use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, post, put},
};
use sha2::{Digest, Sha256};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use aspectus_auth::{
    ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier, TokenVerifier,
};
use aspectus_server::AppState;
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgAuthorizationCodeStore, PgOAuth2ClientStore,
    PgRefreshTokenStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore, PgUserStore,
};
use aspectus_server::email::LoggingEmailSender;
use aspectus_server::middleware::auth::{require_admin_service_token, service_token_auth};

const SERVICE_TOKEN: &str = "aspectus-dev-pandaria-service-token";
const ADMIN_SERVICE_TOKEN: &str = "aspectus-dev-admin-service-token";

pub async fn build_app() -> anyhow::Result<(Router, String)> {
    build_app_with().await.map(|t| (t.router, t.token_hash))
}

/// Bundle returned by [`build_app_with`] — gives tests access to the
/// router plus the underlying JwtSigner / ApiKeyCreator so they can
/// mint JWTs and Opaque tokens without going through the HTTP layer.
pub struct TestApp {
    pub router: Router,
    pub token_hash: String,
    pub jwt_signer: Arc<aspectus_auth::jwt::JwtSigner>,
    pub api_key_creator: Arc<aspectus_auth::ApiKeyCreator>,
}

pub async fn build_app_with() -> anyhow::Result<TestApp> {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());

    let pool = sqlx::PgPool::connect(&db_url).await?;

    // Seed service token using ON CONFLICT DO NOTHING — relies on the
    // migration `20260531000003_seed_service_tokens.sql` having already
    // inserted this token. The token value (`SERVICE_TOKEN` above) MUST
    // match the value seeded by migration #3 so this INSERT is a no-op
    // when migrations have been applied.
    //
    // If you see `401 Invalid Service Token` from these tests, you likely
    // need to re-run migrations:
    //   sqlx migrate run
    let token_hash = hex::encode(Sha256::digest(SERVICE_TOKEN.as_bytes()));
    let _ = sqlx::query(
        "INSERT INTO service_tokens (project, token_hash) VALUES ($1, $2) ON CONFLICT (project) DO NOTHING",
    )
    .bind(aspectus_core::project::Project::Pandaria)
    .bind(&token_hash)
    .execute(&pool)
    .await;

    // Seed internal admin service token for management endpoints.
    let admin_token_hash = hex::encode(Sha256::digest(ADMIN_SERVICE_TOKEN.as_bytes()));
    let _ = sqlx::query(
        "INSERT INTO service_tokens (project, token_hash) VALUES ('aspectus', $1) ON CONFLICT (project) DO NOTHING",
    )
    .bind(&admin_token_hash)
    .execute(&pool)
    .await;

    let redis_client = redis::Client::open(redis_url.as_str())?;
    let auth_cache = RedisCache::new(redis_client.clone()).await?;
    let jwt_cache = RedisCache::new(redis_client.clone()).await?;
    let svc_token_cache = RedisCache::new(redis_client).await?;

    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));
    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), auth_cache));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));
    let api_key_creator_for_test = api_key_creator.clone();

    let service_token_store = Arc::new(PgServiceTokenStore::new(pool.clone()));

    let svc_verifier = Arc::new(ServiceTokenVerifier::new(
        service_token_store.clone(),
        svc_token_cache,
    ));

    let jwt_signer = Arc::new(JwtSigner::from_env()?);
    let jwt_signer_for_test = jwt_signer.clone();
    let jwt_verifier = Arc::new(JwtVerifier::from_env(jwt_cache)?);

    let scope_cache = Arc::new(RedisCache::new(redis::Client::open(redis_url.as_str())?).await?);

    let state = AppState {
        tenant_store: Arc::new(PgTenantStore::new(pool.clone())),
        service_account_store: Arc::new(PgServiceAccountStore::new(pool.clone())),
        api_key_store: api_key_store.clone(),
        audit_log_store: Arc::new(PgAuditLogStore::new(pool.clone())),
        service_token_store: service_token_store.clone(),
        user_store: Arc::new(PgUserStore::new(pool.clone())),
        auth_code_store: Arc::new(PgAuthorizationCodeStore::new(pool.clone())),
        refresh_token_store: Arc::new(PgRefreshTokenStore::new(pool.clone())),
        oauth_client_store: Arc::new(PgOAuth2ClientStore::new(pool.clone())),
        scope_cache,
        email_sender: Arc::new(LoggingEmailSender),
        api_key_creator,
        api_key_verifier: api_key_verifier.clone(),
        token_verifier: Arc::new(TokenVerifier::new(
            api_key_verifier.clone(),
            jwt_verifier.clone(),
        )),
        svc_token_verifier: svc_verifier.clone(),
        jwt_signer: jwt_signer.clone(),
        jwt_verifier,
        pool: pool.clone(),
    };

    let auth_layer = middleware::from_fn(
        move |mut req: axum::extract::Request, next: middleware::Next| {
            let verifier = svc_verifier.clone();
            async move {
                req.extensions_mut().insert(verifier);
                service_token_auth(req, next).await
            }
        },
    );

    let mgmt = Router::new()
        .route("/tenants", post(aspectus_server::routes::tenants::create))
        .route("/tenants/{id}", get(aspectus_server::routes::tenants::get))
        .route(
            "/tenants/{id}/quotas",
            put(aspectus_server::routes::tenants::update_quotas),
        )
        .route(
            "/service-accounts",
            post(aspectus_server::routes::service_accounts::create)
                .get(aspectus_server::routes::service_accounts::list),
        )
        .route(
            "/service-accounts/{id}",
            get(aspectus_server::routes::service_accounts::get),
        )
        .route(
            "/users",
            post(aspectus_server::routes::users::create).get(aspectus_server::routes::users::list),
        )
        .route("/users/{id}", get(aspectus_server::routes::users::get))
        .route(
            "/users/{id}/suspend",
            put(aspectus_server::routes::users::suspend),
        )
        .route("/roles", get(aspectus_server::routes::roles::list))
        .route(
            "/users/{id}/roles",
            post(aspectus_server::routes::roles::assign),
        )
        .route(
            "/users/{id}/roles/{role_id}",
            delete(aspectus_server::routes::roles::remove),
        )
        .route(
            "/api-keys",
            post(aspectus_server::routes::api_keys::create)
                .get(aspectus_server::routes::api_keys::list),
        )
        .route(
            "/api-keys/{id}",
            get(aspectus_server::routes::api_keys::get),
        )
        .route(
            "/api-keys/{id}",
            delete(aspectus_server::routes::api_keys::revoke),
        )
        .route(
            "/clients",
            post(aspectus_server::routes::oauth::create_client)
                .get(aspectus_server::routes::oauth::list_clients),
        )
        .route(
            "/service-tokens",
            post(aspectus_server::routes::service_tokens::create)
                .get(aspectus_server::routes::service_tokens::list),
        )
        .route(
            "/service-tokens/{project}",
            get(aspectus_server::routes::service_tokens::get)
                .delete(aspectus_server::routes::service_tokens::revoke),
        )
        .route(
            "/service-tokens/{project}/rotate",
            post(aspectus_server::routes::service_tokens::rotate),
        )
        .route(
            "/audit-logs",
            get(aspectus_server::routes::audit_logs::list),
        )
        .layer(middleware::from_fn(require_admin_service_token))
        .layer(auth_layer.clone())
        .with_state(state.clone());

    let app = Router::new()
        .route(
            "/introspect",
            post(aspectus_server::routes::introspect::handle),
        )
        .route_layer(auth_layer)
        .route("/health", get(aspectus_server::routes::health::handle))
        .route("/metrics", get(aspectus_server::routes::metrics::handle))
        .route(
            "/.well-known/jwks.json",
            get(aspectus_server::routes::token::jwks),
        )
        .route(
            "/authorize",
            post(aspectus_server::routes::oauth::authorize),
        )
        .route("/oauth/token", post(aspectus_server::routes::oauth::token))
        .route("/login", post(aspectus_server::routes::auth::login))
        .route(
            "/login/lookup",
            post(aspectus_server::routes::auth::login_lookup),
        )
        .with_state(state.clone())
        .merge(mgmt)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(1024 * 16))
        .layer(TraceLayer::new_for_http());

    Ok(TestApp {
        router: app,
        token_hash,
        jwt_signer: jwt_signer_for_test,
        api_key_creator: api_key_creator_for_test,
    })
}

pub fn service_token_header() -> String {
    format!("Bearer {SERVICE_TOKEN}")
}

pub fn admin_service_token_header() -> String {
    format!("Bearer {ADMIN_SERVICE_TOKEN}")
}
