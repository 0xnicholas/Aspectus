use std::net::SocketAddr;
use std::sync::Arc;

use axum::{middleware, Router, extract::DefaultBodyLimit, routing::{delete, get, post, put}};

use aspectus_auth::password::PasswordHasher;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier};
use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use aspectus_server::config::Config;
use aspectus_server::db;
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore, PgUserStore,
};
use aspectus_server::middleware::auth::service_token_auth;
use aspectus_server::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aspectus_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = db::init_pool(&config).await?;

    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let auth_cache = RedisCache::new(redis_client.clone()).await;
    let jwt_cache = RedisCache::new(redis_client).await;

    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));

    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), auth_cache));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));

    let svc_token_verifier = Arc::new(ServiceTokenVerifier::new(
        Arc::new(PgServiceTokenStore::new(pool.clone())),
        RedisCache::new(redis::Client::open(config.redis_url.as_str())?).await,
    ));

    // JWT (optional — only if PEM keys configured)
    let jwt_signer = Arc::new(JwtSigner::from_env().unwrap_or_else(|_| {
        tracing::warn!("JWT not configured (set JWT_PRIVATE_KEY_PEM)");
        panic!("JWT_PRIVATE_KEY_PEM required for v0.4")
    }));
    let jwt_verifier = Arc::new(JwtVerifier::from_env(jwt_cache).unwrap_or_else(|_| {
        panic!("JWT_PUBLIC_KEY_PEM required for v0.4")
    }));

    let state = AppState {
        tenant_store: Arc::new(PgTenantStore::new(pool.clone())),
        service_account_store: Arc::new(PgServiceAccountStore::new(pool.clone())),
        api_key_store: api_key_store.clone(),
        audit_log_store: Arc::new(PgAuditLogStore::new(pool.clone())),
        user_store: Arc::new(PgUserStore::new(pool.clone())),        api_key_creator,
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

    let mgmt = Router::new()
        .route("/tenants", post(aspectus_server::routes::tenants::create))
        .route("/tenants/{id}", get(aspectus_server::routes::tenants::get))
        .route("/tenants/{id}/quotas", put(aspectus_server::routes::tenants::update_quotas))
        .route("/service-accounts", post(aspectus_server::routes::service_accounts::create).get(aspectus_server::routes::service_accounts::list))
        .route("/users", post(aspectus_server::routes::users::create).get(aspectus_server::routes::users::list))
        .route("/users/{id}", get(aspectus_server::routes::users::get))
        .route("/users/{id}/suspend", put(aspectus_server::routes::users::suspend))        .route("/service-accounts/{id}", get(aspectus_server::routes::service_accounts::get))
        .route("/roles", get(aspectus_server::routes::roles::list))
        .route("/users/{id}/roles", post(aspectus_server::routes::roles::assign))
        .route("/users/{id}/roles/{role_id}", delete(aspectus_server::routes::roles::remove))        .route("/users", post(aspectus_server::routes::users::create).get(aspectus_server::routes::users::list))
        .route("/users/{id}", get(aspectus_server::routes::users::get))
        .route("/users/{id}/suspend", put(aspectus_server::routes::users::suspend))        .route("/api-keys", post(aspectus_server::routes::api_keys::create).get(aspectus_server::routes::api_keys::list))
        .route("/roles", get(aspectus_server::routes::roles::list))
        .route("/users/{id}/roles", post(aspectus_server::routes::roles::assign))
        .route("/users/{id}/roles/{role_id}", delete(aspectus_server::routes::roles::remove))        .route("/api-keys/{id}", get(aspectus_server::routes::api_keys::get))
        .route("/api-keys/{id}", delete(aspectus_server::routes::api_keys::revoke))
        .route("/token", post(aspectus_server::routes::token::issue))
        .route("/token/revoke", post(aspectus_server::routes::token::revoke))
        .layer(auth_layer.clone())
        .with_state(state.clone());

    let app = Router::new()
        .route("/introspect", post(aspectus_server::routes::introspect::handle))
        .route("/health", get(aspectus_server::routes::health::handle))
        .route("/.well-known/jwks.json", get(aspectus_server::routes::token::jwks))
        .route("/authorize", post(aspectus_server::routes::oauth::authorize))
        .route("/token", post(aspectus_server::routes::oauth::token))        .route_layer(auth_layer)
        .route("/clients", post(aspectus_server::routes::oauth::create_client).get(aspectus_server::routes::oauth::list_clients))        .with_state(state)
        .merge(mgmt)
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::max(1024 * 16))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Aspectus v{} starting on {}", env!("CARGO_PKG_VERSION"), addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down gracefully...");
}
