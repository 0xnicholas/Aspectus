use std::net::SocketAddr;
use std::sync::Arc;

use axum::{middleware, Router, routing::{delete, get, post, put}};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aspectus_auth::{ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier};
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore,
};

use aspectus_server::config::Config;
use aspectus_server::db;
use aspectus_server::middleware::auth::service_token_auth;
use aspectus_server::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aspectus_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let pool = db::init_pool(&config.database_url).await?;

    // Redis
    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let cache = RedisCache::new(redis_client).await;

    // Stores
    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));

    // Auth
    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), cache.clone()));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));
    let svc_token_verifier =
        Arc::new(ServiceTokenVerifier::new(
            Arc::new(PgServiceTokenStore::new(pool.clone())),
            cache,
        ));

    let state = AppState {
        tenant_store: Arc::new(PgTenantStore::new(pool.clone())),
        service_account_store: Arc::new(PgServiceAccountStore::new(pool.clone())),
        api_key_store: api_key_store.clone(),
        audit_log_store: Arc::new(PgAuditLogStore::new(pool.clone())),
        api_key_creator,
        api_key_verifier,
        svc_token_verifier: svc_token_verifier.clone(),
        pool: pool.clone(),
    };

    // Auth middleware closure
    let svc_verifier = svc_token_verifier;
    let auth_layer = middleware::from_fn(move |mut req: axum::extract::Request, next: middleware::Next| {
        let verifier = svc_verifier.clone();
        async move {
            req.extensions_mut().insert(verifier);
            service_token_auth(req, next).await
        }
    });

    // Management API router
    let mgmt = Router::new()
        .route("/tenants", post(aspectus_server::routes::tenants::create))
        .route("/tenants/{id}", get(aspectus_server::routes::tenants::get))
        .route("/tenants/{id}/quotas", put(aspectus_server::routes::tenants::update_quotas))
        .route(
            "/service-accounts",
            post(aspectus_server::routes::service_accounts::create)
                .get(aspectus_server::routes::service_accounts::list),
        )
        .route("/service-accounts/{id}", get(aspectus_server::routes::service_accounts::get))
        .route(
            "/api-keys",
            post(aspectus_server::routes::api_keys::create)
                .get(aspectus_server::routes::api_keys::list),
        )
        .route("/api-keys/{id}", get(aspectus_server::routes::api_keys::get))
        .route("/api-keys/{id}", delete(aspectus_server::routes::api_keys::revoke))
        .layer(auth_layer.clone())
        .with_state(state.clone());

    // Main router
    let app = Router::new()
        .route("/introspect", post(aspectus_server::routes::introspect::handle))
        .route("/health", get(aspectus_server::routes::health::handle))
        .route_layer(auth_layer)
        .with_state(state)
        .merge(mgmt)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Aspectus v{} starting on {}", env!("CARGO_PKG_VERSION"), addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
