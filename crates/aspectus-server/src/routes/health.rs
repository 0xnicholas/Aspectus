use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize, Default)]
pub struct HealthQuery {
    /// When `true`, also check PostgreSQL and Redis connectivity.
    /// K8s liveness probe: omit (cheap, just returns "ok").
    /// K8s readiness probe: set `full=true` (checks all dependencies).
    #[serde(default)]
    full: bool,
}

/// GET /health
///
/// Light mode (default): `{"status":"ok","version":"0.9.0"}` — always 200.
/// Full mode (`?full=true`): probes PostgreSQL + Redis, returns 200 (ok/degraded)
/// or 503 (down — cannot serve traffic).
pub async fn handle(
    State(state): State<AppState>,
    Query(query): Query<HealthQuery>,
) -> impl IntoResponse {
    let version = env!("CARGO_PKG_VERSION");

    if !query.full {
        return (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "version": version,
            })),
        )
            .into_response();
    }

    // PostgreSQL: a simple `SELECT 1` verifies the connection pool is alive.
    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.pool).await.is_ok();

    // Redis: PING via the verifier's health check.
    let redis_ok = state.api_key_verifier.cache_health().await.is_ok();

    let status = match (db_ok, redis_ok) {
        (true, true) => "ok",
        (true, false) => "degraded", // Redis down → PG fallback works
        (false, _) => "down",        // PG dead → cannot serve anything
    };

    let http_status = if status == "down" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };

    (
        http_status,
        Json(json!({
            "status": status,
            "version": version,
            "postgres": if db_ok { "ok" } else { "error" },
            "redis": if redis_ok { "ok" } else { "error" },
        })),
    )
        .into_response()
}
