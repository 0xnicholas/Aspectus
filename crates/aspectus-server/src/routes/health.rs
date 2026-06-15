use axum::{extract::{Query, State}, Json};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize, Default)]
pub struct HealthQuery {
    /// When true, also check PostgreSQL and Redis connectivity.
    /// Default (no param): lightweight check only (always "ok").
    #[serde(default)]
    full: bool,
}

pub async fn handle(
    State(state): State<AppState>,
    Query(query): Query<HealthQuery>,
) -> Json<serde_json::Value> {
    if !query.full {
        return Json(json!({"status": "ok"}));
    }

    let db_ok = sqlx::query("SELECT 1").fetch_one(&state.pool).await.is_ok();
    let redis_ok = state
        .api_key_verifier
        .cache_health()
        .await
        .is_ok();

    Json(json!({
        "status": if db_ok && redis_ok { "ok" } else { "degraded" },
        "postgres": if db_ok { "ok" } else { "error" },
        "redis": if redis_ok { "ok" } else { "error" },
    }))
}
