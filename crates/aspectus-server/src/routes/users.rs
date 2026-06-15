use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use aspectus_core::{
    store::AuditLogStore,
    audit_log::AuditLog,
    identity::IdentityType,
    store::UserStore,
};

use crate::error::ProblemDetails;
use crate::AppState;
use aspectus_auth::password::PasswordHasher;

use crate::util::generate_id;

/// Validate email format (basic check).
fn validate_email(email: &str) -> bool {
    email.contains('@') && email.len() <= 256 && !email.contains('\0')
}

/// Validate display name: ≤128 chars, no control characters.
fn validate_display_name(name: &str) -> bool {
    name.len() <= 128 && !name.chars().any(|c| c.is_control())
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    tenant_id: String,
    email: String,
    password: String,
    display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    tenant_id: String,
}

#[derive(Deserialize)]
pub struct SuspendRequest {
    suspended: bool,
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if !validate_email(&req.email) {
        return ProblemDetails::validation_failed("Invalid email format", vec![]).into_response();
    }
    if let Some(ref display_name) = req.display_name
        && !validate_display_name(display_name)
    {
        return ProblemDetails::validation_failed("Invalid display name", vec![]).into_response();
    }
    if req.password.len() < 8 {
        return ProblemDetails::validation_failed("Password must be at least 8 characters", vec![])
            .into_response();
    }

    let hash = match PasswordHasher::hash(&req.password) {
        Ok(h) => h,
        Err(e) => return ProblemDetails::internal_error(e).into_response(),
    };

    match state.user_store.create(
        &req.tenant_id, &req.email, &hash, req.display_name.as_deref(),
    ).await {
        Ok(user) => {
            // Auto-assign default roles
            if let Ok(roles) = sqlx::query_as::<_, (String,)>(
                "SELECT id FROM roles WHERE is_default = true AND type IN ('user','both')",
            )
            .fetch_all(&state.pool)
            .await
            {
                for (role_id,) in &roles {
                    let _ = sqlx::query(
                        "INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
                    )
                    .bind(generate_id())
                    .bind(&user.id)
                    .bind(role_id)
                    .execute(&state.pool)
                    .await;
                }
            }

            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(), tenant_id: user.tenant_id.clone(),
                actor_id: "mgmt".into(), actor_type: IdentityType::ServiceAccount,
                action: "user.created".into(), target_type: "user".into(),
                target_id: user.id.clone(), metadata: json!({"email": &user.email}),
                created_at: Utc::now(),
            }).await;

            (StatusCode::CREATED, Json(user)).into_response()
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("users__email") || msg.contains("unique") {
                ProblemDetails::validation_failed("Email already exists in this tenant", vec![]).into_response()
            } else {
                ProblemDetails::from(e).into_response()
            }
        }
    }
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.user_store.get_by_id(&id).await {
        Ok(Some(user)) => Json(user).into_response(),
        Ok(None) => ProblemDetails::not_found(format!("User {id} not found")).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    match state.user_store.list_by_tenant(&query.tenant_id).await {
        Ok(users) => (StatusCode::OK, Json(users)).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

pub async fn suspend(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SuspendRequest>,
) -> impl IntoResponse {
    match state.user_store.set_suspended(&id, req.suspended).await {
        Ok(true) => {
            let _ = state.audit_log_store.append(AuditLog {
                id: generate_id(), tenant_id: String::new(),
                actor_id: "mgmt".into(), actor_type: IdentityType::ServiceAccount,
                action: "user.suspended".into(), target_type: "user".into(),
                target_id: id, metadata: json!({"suspended": req.suspended}),
                created_at: Utc::now(),
            }).await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => ProblemDetails::not_found("User not found").into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}
