use std::net::SocketAddr;

use axum::{Router, routing::get};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aspectus_server::config::Config;
use aspectus_server::db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env (development only — silently ignored if missing)
    let _ = dotenvy::dotenv();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aspectus_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = Config::from_env()?;

    // Initialize database connection pool
    let _db_pool = db::init_pool(&config.database_url).await?;

    // Build router
    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    // Bind and serve
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!(
        "Aspectus v{} starting on {}",
        env!("CARGO_PKG_VERSION"),
        addr
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
