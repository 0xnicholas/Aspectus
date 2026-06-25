use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::http::header;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::HeaderValue,
    middleware,
    routing::{delete, get, post, put},
};
use sha2::{Digest, Sha256};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aspectus_auth::jwt::{JwtSigner, JwtVerifier};
use aspectus_auth::{
    ApiKeyCreator, ApiKeyVerifier, RedisCache, ServiceTokenVerifier, TokenVerifier,
};
use aspectus_server::AppState;
use aspectus_server::cleanup::spawn_cleanup_task;
use aspectus_server::config::Config;
use aspectus_server::db;
use aspectus_server::db::{
    PgApiKeyStore, PgAuditLogStore, PgAuthorizationCodeStore, PgOAuth2ClientStore,
    PgRefreshTokenStore, PgServiceAccountStore, PgServiceTokenStore, PgTenantStore, PgUserStore,
};
use aspectus_server::email::{EmailSender, LoggingEmailSender, SmtpEmailSender};
use aspectus_server::middleware::audit::audit_layer;
use aspectus_server::middleware::auth::{require_admin_service_token, service_token_auth};
use aspectus_server::middleware::rate_limit::{self, RateLimiter};

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

    // Seed the internal admin service token if ASPECTUS_ADMIN_SERVICE_TOKEN is set.
    // Management endpoints (/tenants, /users, /api-keys, ...) require this token.
    // In production it must be a strong, rotated secret injected via env/secrets manager.
    if let Ok(admin_token) = std::env::var("ASPECTUS_ADMIN_SERVICE_TOKEN") {
        if !admin_token.is_empty() {
            let admin_hash = Sha256::digest(admin_token.as_bytes());
            let admin_hash_hex = hex::encode(admin_hash);
            match sqlx::query(
                "INSERT INTO service_tokens (project, token_hash) VALUES ('aspectus', $1)
                 ON CONFLICT (project) DO UPDATE SET token_hash = EXCLUDED.token_hash, updated_at = NOW()",
            )
            .bind(&admin_hash_hex)
            .execute(&pool)
            .await
            {
                Ok(_) => tracing::info!("Admin service token seeded for project 'aspectus'"),
                Err(e) => tracing::error!(error = %e, "Failed to seed admin service token"),
            }
        } else {
            tracing::warn!(
                "ASPECTUS_ADMIN_SERVICE_TOKEN is set but empty; management endpoints will be inaccessible"
            );
        }
    } else {
        tracing::warn!(
            "ASPECTUS_ADMIN_SERVICE_TOKEN not set; management endpoints will be inaccessible"
        );
    }

    let redis_client =
        redis::Client::open(config.redis_url.as_str()).context("Failed to create Redis client")?;
    let auth_cache = RedisCache::new(redis_client.clone())
        .await
        .context("Failed to create auth Redis cache")?;
    let jwt_cache = RedisCache::new(redis_client.clone())
        .await
        .context("Failed to create JWT Redis cache")?;

    let api_key_store = Arc::new(PgApiKeyStore::new(pool.clone()));
    let api_key_verifier = Arc::new(ApiKeyVerifier::new(api_key_store.clone(), auth_cache));
    let api_key_creator = Arc::new(ApiKeyCreator::new(api_key_store.clone()));

    let svc_token_cache = RedisCache::new(redis_client.clone())
        .await
        .context("Failed to create service token Redis cache")?;

    let scope_cache = Arc::new(
        RedisCache::new(redis_client.clone())
            .await
            .context("Failed to create scope expansion Redis cache")?,
    );

    let service_token_store = Arc::new(PgServiceTokenStore::new(pool.clone()));

    let svc_token_verifier = Arc::new(ServiceTokenVerifier::new(
        service_token_store.clone(),
        svc_token_cache,
    ));

    let jwt_signer = Arc::new(
        JwtSigner::from_env()
            .context("JWT_PRIVATE_KEY_PEM required — provide via env var or file")?,
    );
    let jwt_verifier = Arc::new(
        JwtVerifier::from_env(jwt_cache)
            .context("JWT_PUBLIC_KEY_PEM required — provide via env var or file")?,
    );

    // Email transport: SMTP in production, logging (stub) in dev/test.
    let email_sender: Arc<dyn EmailSender> =
        match std::env::var("ASPECTUS_EMAIL_TRANSPORT").ok().as_deref() {
            Some("smtp") => Arc::new(SmtpEmailSender::from_env()?),
            Some(other) => {
                tracing::warn!(
                    transport = %other,
                    "Unknown ASPECTUS_EMAIL_TRANSPORT value; falling back to logging transport"
                );
                Arc::new(LoggingEmailSender)
            }
            None => Arc::new(LoggingEmailSender),
        };

    // Redis-backed rate limiters (cluster-wide, shared across replicas).
    let authorize_limiter = RateLimiter::new(redis_client.clone(), 5, 60)
        .await
        .context("Failed to create authorize rate limiter")?;
    let password_limiter = RateLimiter::new(redis_client.clone(), 3, 60)
        .await
        .context("Failed to create password rate limiter")?;
    let token_limiter = RateLimiter::new(redis_client.clone(), 30, 60)
        .await
        .context("Failed to create token rate limiter")?;
    let introspect_limiter = RateLimiter::new(redis_client.clone(), 10000, 60)
        .await
        .context("Failed to create introspect rate limiter")?;
    let mgmt_limiter = RateLimiter::new(redis_client.clone(), 100, 60)
        .await
        .context("Failed to create management rate limiter")?;

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
        scope_cache: scope_cache.clone(),
        email_sender: email_sender.clone(),
        api_key_creator,
        api_key_verifier: api_key_verifier.clone(),
        token_verifier: Arc::new(TokenVerifier::new(api_key_verifier, jwt_verifier.clone())),
        svc_token_verifier: svc_token_verifier.clone(),
        jwt_signer,
        jwt_verifier,
        pool: pool.clone(),
    };

    spawn_cleanup_task(pool.clone());

    let auth_svc_verifier = svc_token_verifier.clone();
    let auth_layer = middleware::from_fn(
        move |mut req: axum::extract::Request, next: middleware::Next| {
            let verifier = auth_svc_verifier.clone();
            async move {
                req.extensions_mut().insert(verifier);
                service_token_auth(req, next).await
            }
        },
    );
    let revoke_auth_svc_verifier = svc_token_verifier.clone();

    let authorize_rl = authorize_limiter.clone();
    let login_rl = authorize_limiter.clone();
    let register_rl = authorize_limiter.clone();
    let login_lookup_rl = authorize_limiter.clone();
    let password_rl = password_limiter.clone();
    let password_rl2 = password_limiter.clone();
    let password_rl3 = password_limiter.clone();
    let token_rl = token_limiter.clone();
    let token_rl2 = token_limiter.clone();
    let introspect_rl = introspect_limiter.clone();
    let mgmt_rl = mgmt_limiter.clone();

    // Management API (auth required + rate limited)
    let mgmt = Router::new()
        .route(
            "/tenants",
            post(aspectus_server::routes::tenants::create)
                .get(aspectus_server::routes::tenants::list),
        )
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
        .route(
            "/users/{id}/unlock",
            post(aspectus_server::routes::users::unlock),
        )
        .route(
            "/users/{id}/scopes",
            get(aspectus_server::routes::users::scopes),
        )
        .route("/roles", get(aspectus_server::routes::roles::list))
        .route("/roles", post(aspectus_server::routes::roles::create))
        .route("/roles/{id}", get(aspectus_server::routes::roles::get))
        .route("/roles/{id}", put(aspectus_server::routes::roles::update))
        .route(
            "/roles/{id}",
            delete(aspectus_server::routes::roles::delete),
        )
        .route(
            "/users/{id}/roles",
            get(aspectus_server::routes::users::list_roles)
                .post(aspectus_server::routes::roles::assign),
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
        // Layers are applied bottom-up: auth runs first, then admin check,
        // then audit/rate-limit closer to the handler.
        .layer(middleware::from_fn(move |req, next| {
            rate_limit::rate_limit_layer(mgmt_rl.clone(), rate_limit::service_token_key, req, next)
        }))
        .layer(middleware::from_fn(audit_layer(
            state.audit_log_store.clone(),
        )))
        .layer(middleware::from_fn(require_admin_service_token))
        .layer(auth_layer.clone())
        .with_state(state.clone());

    // Public + introspect routes (with per-route rate limiting)
    let app = Router::new()
        .route(
            "/openapi.yaml",
            get(aspectus_server::routes::docs::openapi_spec),
        )
        .route("/docs", get(aspectus_server::routes::docs::swagger_ui))
        .route(
            "/token/revoke",
            post(aspectus_server::routes::token::revoke).route_layer(middleware::from_fn(
                move |mut req: axum::extract::Request, next: middleware::Next| {
                    let verifier = revoke_auth_svc_verifier.clone();
                    async move {
                        req.extensions_mut().insert(verifier);
                        service_token_auth(req, next).await
                    }
                },
            )),
        )
        .route(
            "/introspect",
            post(aspectus_server::routes::introspect::handle).route_layer(middleware::from_fn(
                move |req, next| {
                    rate_limit::rate_limit_layer(
                        introspect_rl.clone(),
                        rate_limit::service_token_key,
                        req,
                        next,
                    )
                },
            )),
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
            post(aspectus_server::routes::oauth::authorize)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(
                        authorize_rl.clone(),
                        rate_limit::ip_key,
                        req,
                        next,
                    )
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/login",
            post(aspectus_server::routes::auth::login)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(login_rl.clone(), rate_limit::ip_key, req, next)
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/login/lookup",
            post(aspectus_server::routes::auth::login_lookup)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(
                        login_lookup_rl.clone(),
                        rate_limit::ip_key,
                        req,
                        next,
                    )
                }))
                .layer(DefaultBodyLimit::max(2048)),
        )
        .route(
            "/register",
            post(aspectus_server::routes::auth::register)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(register_rl.clone(), rate_limit::ip_key, req, next)
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/logout",
            post(aspectus_server::routes::auth::logout).layer(DefaultBodyLimit::max(2048)),
        )
        .route(
            "/forgot-password",
            post(aspectus_server::routes::auth::forgot_password)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(password_rl.clone(), rate_limit::ip_key, req, next)
                }))
                .layer(DefaultBodyLimit::max(2048)),
        )
        .route(
            "/reset-password",
            post(aspectus_server::routes::auth::reset_password)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(
                        password_rl2.clone(),
                        rate_limit::ip_key,
                        req,
                        next,
                    )
                }))
                .layer(DefaultBodyLimit::max(2048)),
        )
        .route(
            "/users/{id}/change-password",
            post(aspectus_server::routes::users::change_password)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(
                        password_rl3.clone(),
                        rate_limit::ip_key,
                        req,
                        next,
                    )
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/oauth/token",
            post(aspectus_server::routes::oauth::token)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(token_rl.clone(), rate_limit::ip_key, req, next)
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .route(
            "/token",
            post(aspectus_server::routes::token::issue)
                .route_layer(middleware::from_fn(move |req, next| {
                    rate_limit::rate_limit_layer(token_rl2.clone(), rate_limit::ip_key, req, next)
                }))
                .layer(DefaultBodyLimit::max(4096)),
        )
        .with_state(state)
        .merge(mgmt)
        // Admin console (React SPA) — served from the same process.
        // Run `cd console && npm run build` before deploying.
        .nest_service(
            "/admin",
            ServeDir::new("console/dist").fallback(ServeFile::new("console/dist/index.html")),
        )
        .layer(build_cors_layer())
        .layer(DefaultBodyLimit::max(1024 * 16))
        .layer(middleware::from_fn(add_security_headers))
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(
        "Aspectus v{} starting on {}",
        env!("CARGO_PKG_VERSION"),
        addr
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Build the CORS layer based on environment.
///
/// - If `ASPECTUS_CONSOLE_ORIGIN` is set, only that origin is allowed.
/// - In debug builds with no origin configured, fall back to permissive for
///   local development.
/// - Otherwise, deny cross-origin requests by default.
fn build_cors_layer() -> CorsLayer {
    if let Ok(origin) = std::env::var("ASPECTUS_CONSOLE_ORIGIN") {
        if let Ok(header_value) = origin.parse::<HeaderValue>() {
            return CorsLayer::new()
                .allow_origin(header_value)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any);
        }
        tracing::warn!(
            origin = %origin,
            "ASPECTUS_CONSOLE_ORIGIN is not a valid HTTP header value; using default CORS"
        );
    }

    if cfg!(debug_assertions) {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
    }
}

async fn add_security_headers(
    req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    response
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
    tracing::info!("Shutting down gracefully...");
}
