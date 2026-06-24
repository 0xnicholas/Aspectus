use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use aspectus_core::{audit_log::AuditLog, identity::IdentityType, role::Role};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::scope_expander::ScopeExpander;
use crate::util::generate_id;
use aspectus_core::store::AuditLogStore;

#[derive(Deserialize)]
pub struct AssignRoleRequest {
    role_id: String,
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name")
        .fetch_all(&state.pool)
        .await
    {
        Ok(roles) => (StatusCode::OK, Json(roles)).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn assign(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> impl IntoResponse {
    let id = generate_id();
    match sqlx::query("INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(&user_id)
        .bind(&req.role_id)
        .execute(&state.pool)
        .await
    {
        Ok(_) => {
            // Invalidate scope cache for this user
            ScopeExpander::invalidate(&state.scope_cache, &user_id).await;

            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: String::new(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "role.assigned".into(),
                    target_type: "role".into(),
                    target_id: req.role_id,
                    metadata: json!({"user_id": &user_id}),
                    created_at: Utc::now(),
                })
                .await;
            (StatusCode::CREATED, Json(json!({"id": id}))).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("users_roles__role_type") {
                ProblemDetails::validation_failed("Role type mismatch", vec![]).into_response()
            } else {
                ProblemDetails::internal_error(msg).into_response()
            }
        }
    }
}

pub async fn remove(
    State(state): State<AppState>,
    Path((user_id, role_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM users_roles WHERE user_id = $1 AND role_id = $2")
        .bind(&user_id)
        .bind(&role_id)
        .execute(&state.pool)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => {
            // Invalidate scope cache for this user
            ScopeExpander::invalidate(&state.scope_cache, &user_id).await;

            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: String::new(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "role.removed".into(),
                    target_type: "role".into(),
                    target_id: role_id,
                    metadata: json!({"user_id": &user_id}),
                    created_at: Utc::now(),
                })
                .await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(_) => ProblemDetails::not_found("User or role not found").into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}
