use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::{middleware, Router, extract::DefaultBodyLimit, routing::{delete, get, post, put}};
use axum::http::header;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier};
use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use aspectus_server::config::Config;
use aspectus_server::db;
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore, PgUserStore,
    PgAuthorizationCodeStore, PgRefreshTokenStore, PgOAuth2ClientStore,
};
use aspectus_server::middleware::auth::service_token_auth;
use aspectus_server::middleware::audit::audit_layer;
use aspectus_server::middleware::rate_limit::{self, RateLimiter};
use aspectus_server::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "aspectus_server=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = db::init_pool(&config).await?;

    let redis_client = redis::Client::open(config.redis_url.as_str())
        .context("Failed to create Redis client")?;
    let auth_cache = RedisCache::new(redis_client.clone()).await
        .context("Failed to create auth Redis cache")?;
    let jwt_cache = RedisCache::new(redis_client.clone()).await
        .context("Failed to create JWT Redis cache")?;

    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));
    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), auth_cache));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));

    let svc_token_cache = RedisCache::new(redis_client.clone()).await
        .context("Failed to create service token Redis cache")?;

    let scope_cache = Arc::new(RedisCache::new(redis_client.clone()).await
        .context("Failed to create scope expansion Redis cache")?);

    let svc_token_verifier = Arc::new(ServiceTokenVerifier::new(
        Arc::new(PgServiceTokenStore::new(pool.clone())),
        svc_token_cache,
    ));

    let jwt_signer = Arc::new(JwtSigner::from_env()
        .context("JWT_PRIVATE_KEY_PEM required — provide via env var or file")?);
    let jwt_verifier = Arc::new(JwtVerifier::from_env(jwt_cache)
        .context("JWT_PUBLIC_KEY_PEM required — provide via env var or file")?);

    let state = AppState {
        tenant_store: Arc::new(PgTenantStore::new(pool.clone())),
        service_account_store: Arc::new(PgServiceAccountStore::new(pool.clone())),
        api_key_store: api_key_store.clone(),
        audit_log_store: Arc::new(PgAuditLogStore::new(pool.clone())),
        user_store: Arc::new(PgUserStore::new(pool.clone())),
        auth_code_store: Arc::new(PgAuthorizationCodeStore::new(pool.clone())),
        refresh_token_store: Arc::new(PgRefreshTokenStore::new(pool.clone())),
        oauth_client_store: Arc::new(PgOAuth2ClientStore::new(pool.clone())),
        scope_cache: scope_cache.clone(),
        api_key_creator,
        api_key_verifier,
        svc_token_verifier: svc_token_verifier.clone(),
        jwt_signer,
        jwt_verifier,
        pool: pool.clone(),
    };

    let svc_verifier = svc_token_verifier;
    let auth_layer = middleware::from_fn(move |mut req: axum::extract::Request, next: middleware::Next| {
        let verifier = svc_verifier.clone();
        async move {
            req.extensions_mut().insert(verifier);
            service_token_auth(req, next).await
        }
    });

    // Rate limiters (per-endpoint tuning)
    let authorize_limiter = RateLimiter::new(5, 60);     // 5/min per IP
    let password_limiter = RateLimiter::new(3, 60);      // 3/min per IP (password ops)
    let token_limiter = RateLimiter::new(30, 60);        // 30/min per IP
    let introspect_limiter = RateLimiter::new(10000, 60); // 10000/min per service token
    let mgmt_limiter = RateLimiter::new(100, 60);         // 100/min per service token

    let authorize_rl = authorize_limiter.clone();
    let login_rl = authorize_limiter.clone();
    let register_rl = authorize_limiter.clone();
    let login_lookup_rl = authorize_limiter.clone();
    let password_rl = password_limiter.clone();
    let password_rl2 = password_limiter.clone();
    let token_rl = token_limiter.clone();
    let introspect_rl = introspect_limiter.clone();
    let mgmt_rl = mgmt_limiter.clone();

    // Management API (auth required + rate limited)
    let mgmt = Router::new()
        .route("/tenants", post(aspectus_server::routes::tenants::create))
        .route("/tenants/{id}", get(aspectus_server::routes::tenants::get))
        .route("/tenants/{id}/quotas", put(aspectus_server::routes::tenants::update_quotas))
        .route("/service-accounts", post(aspectus_server::routes::service_accounts::create).get(aspectus_server::routes::service_accounts::list))
        .route("/service-accounts/{id}", get(aspectus_server::routes::service_accounts::get))
        .route("/users", post(aspectus_server::routes::users::create).get(aspectus_server::routes::users::list))
        .route("/users/{id}", get(aspectus_server::routes::users::get))
        .route("/users/{id}/suspend", put(aspectus_server::routes::users::suspend))
        .route("/roles", get(aspectus_server::routes::roles::list))
        .route("/users/{id}/roles", post(aspectus_server::routes::roles::assign))
        .route("/users/{id}/roles/{role_id}", delete(aspectus_server::routes::roles::remove))
        .route("/api-keys", post(aspectus_server::routes::api_keys::create).get(aspectus_server::routes::api_keys::list))
        .route("/api-keys/{id}", get(aspectus_server::routes::api_keys::get))
        .route("/api-keys/{id}", delete(aspectus_server::routes::api_keys::revoke))
        .route("/clients", post(aspectus_server::routes::oauth::create_client).get(aspectus_server::routes::oauth::list_clients))
        .layer(auth_layer.clone())
        .layer(middleware::from_fn(audit_layer(state.audit_log_store.clone())))
        .layer(middleware::from_fn(move |req, next| {
            rate_limit::rate_limit_layer(mgmt_rl.clone(), rate_limit::service_token_key, req, next)
        }))
        .with_state(state.clone());

    // Public + introspect routes (with per-route rate limiting)
    let app = Router::new()
        .route("/introspect", post(aspectus_server::routes::introspect::handle)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(introspect_rl.clone(), rate_limit::service_token_key, req, next)
            }))
        )
        .route_layer(auth_layer)
        .route("/health", get(aspectus_server::routes::health::handle))
        .route("/metrics", get(aspectus_server::routes::metrics::handle))
        .route("/.well-known/jwks.json", get(aspectus_server::routes::token::jwks))
        .route("/authorize", post(aspectus_server::routes::oauth::authorize)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(authorize_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(4096))
        )
        .route("/login", post(aspectus_server::routes::auth::login)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(login_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(4096))
        )
        .route("/login/lookup", post(aspectus_server::routes::auth::login_lookup)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(login_lookup_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(2048))
        )
        .route("/register", post(aspectus_server::routes::auth::register)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(register_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(4096))
        )
        .route("/logout", post(aspectus_server::routes::auth::logout)
            .layer(DefaultBodyLimit::max(2048))
        )
        .route("/forgot-password", post(aspectus_server::routes::auth::forgot_password)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(password_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(2048))
        )
        .route("/reset-password", post(aspectus_server::routes::auth::reset_password)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(password_rl2.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(2048))
        )
        .route("/oauth/token", post(aspectus_server::routes::oauth::token)
            .route_layer(middleware::from_fn(move |req, next| {
                rate_limit::rate_limit_layer(token_rl.clone(), rate_limit::ip_key, req, next)
            }))
            .layer(DefaultBodyLimit::max(4096))
        )
        .with_state(state)
        .merge(mgmt)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(1024 * 16))
        .layer(middleware::from_fn(add_security_headers))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Aspectus v{} starting on {}", env!("CARGO_PKG_VERSION"), addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}

async fn add_security_headers(req: axum::extract::Request, next: middleware::Next) -> axum::response::Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    response
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down gracefully...");
}
