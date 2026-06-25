use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use aspectus_core::{
    audit_log::AuditLog, identity::IdentityType, identity::RoleType, role::Role, scope::Scope,
};

use crate::AppState;
use crate::error::ProblemDetails;
use crate::scope_expander::ScopeExpander;
use crate::util::generate_id;
use aspectus_core::store::AuditLogStore;

#[derive(Deserialize)]
pub struct AssignRoleRequest {
    role_id: String,
}

#[derive(Serialize)]
pub struct RoleDetail {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub role_type: String,
    pub is_default: bool,
    pub is_system: bool,
    pub scopes: Vec<String>,
}

async fn fetch_role_scopes(pool: &sqlx::PgPool, role_id: &str) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT s.name FROM scopes s \
         JOIN roles_scopes rs ON rs.scope_id = s.id \
         WHERE rs.role_id = $1 ORDER BY s.name",
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

fn role_type_str(t: RoleType) -> &'static str {
    match t {
        RoleType::User => "user",
        RoleType::ServiceAccount => "service_account",
        RoleType::Both => "both",
    }
}

fn role_to_detail(role: Role, scopes: Vec<String>) -> RoleDetail {
    RoleDetail {
        id: role.id,
        name: role.name,
        description: role.description,
        role_type: role_type_str(role.r#type).to_string(),
        is_default: role.is_default,
        is_system: role.is_system,
        scopes,
    }
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let roles = match sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name")
        .fetch_all(&state.pool)
        .await
    {
        Ok(roles) => roles,
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    let mut details = Vec::with_capacity(roles.len());
    for role in roles {
        match fetch_role_scopes(&state.pool, &role.id).await {
            Ok(scopes) => details.push(role_to_detail(role, scopes)),
            Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
        }
    }
    (StatusCode::OK, Json(details)).into_response()
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let role = match sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = $1")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return ProblemDetails::not_found(format!("Role {id} not found")).into_response();
        }
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    match fetch_role_scopes(&state.pool, &id).await {
        Ok(scopes) => (StatusCode::OK, Json(role_to_detail(role, scopes))).into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}

async fn tenant_id_for_user(pool: &sqlx::PgPool, user_id: &str) -> Result<String, ProblemDetails> {
    match sqlx::query_as::<_, (String,)>("SELECT tenant_id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some((tenant_id,))) => Ok(tenant_id),
        Ok(None) => Err(ProblemDetails::not_found(format!(
            "User {user_id} not found"
        ))),
        Err(e) => Err(ProblemDetails::internal_error(e.to_string())),
    }
}

pub async fn assign(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> impl IntoResponse {
    let tenant_id = match tenant_id_for_user(&state.pool, &user_id).await {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

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
                    tenant_id,
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
            } else if msg.contains("users_roles__unique") {
                // Idempotent: assignment already exists.
                StatusCode::NO_CONTENT.into_response()
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
    let tenant_id = match tenant_id_for_user(&state.pool, &user_id).await {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };

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
                    tenant_id,
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

#[derive(Deserialize)]
pub struct CreateRoleRequest {
    name: String,
    description: Option<String>,
    #[serde(rename = "type")]
    role_type: String,
    scopes: Vec<String>,
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    description: Option<String>,
    #[serde(rename = "type")]
    role_type: String,
    scopes: Vec<String>,
}

fn validate_role_name(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.len() > 128 {
        return Some("Role name must be between 1 and 128 characters");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Some("Role name may only contain letters, numbers, underscore, and hyphen");
    }
    None
}

fn parse_role_type(s: &str) -> Option<RoleType> {
    RoleType::try_from(s).ok()
}

fn validate_role_scopes(scopes: &[String]) -> Option<String> {
    if scopes.is_empty() {
        return Some("Role must contain at least one scope".into());
    }
    for scope in scopes {
        if let Err(e) = Scope::validate(scope) {
            return Some(e.to_string());
        }
    }
    None
}

async fn insert_role_scopes(
    pool: &sqlx::PgPool,
    role_id: &str,
    scopes: &[String],
) -> Result<(), sqlx::Error> {
    for scope in scopes {
        let scope_row: Option<(String,)> = sqlx::query_as("SELECT id FROM scopes WHERE name = $1")
            .bind(scope)
            .fetch_optional(pool)
            .await?;
        let scope_id = match scope_row {
            Some((id,)) => id,
            None => {
                // Auto-create unknown scope so custom roles are not blocked
                // when a new project introduces a scope before it is seeded.
                let id = generate_id();
                sqlx::query("INSERT INTO scopes (id, name) VALUES ($1, $2)")
                    .bind(&id)
                    .bind(scope)
                    .execute(pool)
                    .await?;
                id
            }
        };
        sqlx::query(
            "INSERT INTO roles_scopes (id, role_id, scope_id) VALUES ($1, $2, $3) \
             ON CONFLICT (role_id, scope_id) DO NOTHING",
        )
        .bind(generate_id())
        .bind(role_id)
        .bind(scope_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn invalidate_users_with_role(
    pool: &sqlx::PgPool,
    cache: &aspectus_auth::RedisCache,
    role_id: &str,
) {
    let users: Vec<(String,)> =
        sqlx::query_as("SELECT user_id FROM users_roles WHERE role_id = $1")
            .bind(role_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    for (user_id,) in users {
        ScopeExpander::invalidate(cache, &user_id).await;
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateRoleRequest>,
) -> impl IntoResponse {
    if let Some(msg) = validate_role_name(&req.name) {
        return ProblemDetails::validation_failed(msg, vec![]).into_response();
    }
    if let Some(msg) = validate_role_scopes(&req.scopes) {
        return ProblemDetails::validation_failed(msg, vec![]).into_response();
    }
    let role_type = match parse_role_type(&req.role_type) {
        Some(t) => t,
        None => {
            return ProblemDetails::validation_failed(
                "Role type must be one of: user, service_account, both",
                vec![],
            )
            .into_response();
        }
    };

    let id = generate_id();
    let role = match sqlx::query_as::<_, Role>(
        "INSERT INTO roles (id, name, description, type, is_default, is_system) \
         VALUES ($1, $2, $3, $4, false, false) RETURNING *",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(role_type)
    .fetch_one(&state.pool)
    .await
    {
        Ok(role) => role,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("roles__name") || msg.contains("unique") {
                return ProblemDetails::validation_failed("Role name already exists", vec![])
                    .into_response();
            }
            return ProblemDetails::internal_error(msg).into_response();
        }
    };

    if let Err(e) = insert_role_scopes(&state.pool, &id, &req.scopes).await {
        return ProblemDetails::internal_error(e.to_string()).into_response();
    }

    let scopes = match fetch_role_scopes(&state.pool, &id).await {
        Ok(s) => s,
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    let _ = state
        .audit_log_store
        .append(AuditLog {
            id: generate_id(),
            tenant_id: String::new(),
            actor_id: "mgmt".into(),
            actor_type: IdentityType::ServiceAccount,
            action: "role.created".into(),
            target_type: "role".into(),
            target_id: id.clone(),
            metadata: json!({"name": &req.name, "scopes": &req.scopes}),
            created_at: Utc::now(),
        })
        .await;

    (StatusCode::CREATED, Json(role_to_detail(role, scopes))).into_response()
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRoleRequest>,
) -> impl IntoResponse {
    if let Some(msg) = validate_role_scopes(&req.scopes) {
        return ProblemDetails::validation_failed(msg, vec![]).into_response();
    }
    let role_type = match parse_role_type(&req.role_type) {
        Some(t) => t,
        None => {
            return ProblemDetails::validation_failed(
                "Role type must be one of: user, service_account, both",
                vec![],
            )
            .into_response();
        }
    };

    let role = match sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = $1")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return ProblemDetails::not_found(format!("Role {id} not found")).into_response();
        }
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    if role.is_system {
        return ProblemDetails::forbidden("System roles cannot be modified").into_response();
    }

    let role = match sqlx::query_as::<_, Role>(
        "UPDATE roles SET description = $1, type = $2 WHERE id = $3 RETURNING *",
    )
    .bind(&req.description)
    .bind(role_type)
    .bind(&id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(role) => role,
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    if let Err(e) = sqlx::query("DELETE FROM roles_scopes WHERE role_id = $1")
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        return ProblemDetails::internal_error(e.to_string()).into_response();
    }

    if let Err(e) = insert_role_scopes(&state.pool, &id, &req.scopes).await {
        return ProblemDetails::internal_error(e.to_string()).into_response();
    }

    invalidate_users_with_role(&state.pool, state.scope_cache.as_ref(), &id).await;

    let scopes = match fetch_role_scopes(&state.pool, &id).await {
        Ok(s) => s,
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    let _ = state
        .audit_log_store
        .append(AuditLog {
            id: generate_id(),
            tenant_id: String::new(),
            actor_id: "mgmt".into(),
            actor_type: IdentityType::ServiceAccount,
            action: "role.updated".into(),
            target_type: "role".into(),
            target_id: id.clone(),
            metadata: json!({"scopes": &req.scopes}),
            created_at: Utc::now(),
        })
        .await;

    (StatusCode::OK, Json(role_to_detail(role, scopes))).into_response()
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let mut tx = match state.pool.begin().await {
        Ok(tx) => tx,
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    let role = match sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = $1")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return ProblemDetails::not_found(format!("Role {id} not found")).into_response();
        }
        Err(e) => return ProblemDetails::internal_error(e.to_string()).into_response(),
    };

    if role.is_system {
        return ProblemDetails::forbidden("System roles cannot be deleted").into_response();
    }

    let assignments: Vec<(String,)> =
        sqlx::query_as("SELECT user_id FROM users_roles WHERE role_id = $1 LIMIT 1")
            .bind(&id)
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_default();
    if !assignments.is_empty() {
        return ProblemDetails::validation_failed(
            "Cannot delete role while it is assigned to users",
            vec![],
        )
        .into_response();
    }

    if let Err(e) = sqlx::query("DELETE FROM roles_scopes WHERE role_id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
    {
        return ProblemDetails::internal_error(e.to_string()).into_response();
    }

    match sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(&id)
        .execute(&mut *tx)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => {
            if let Err(e) = tx.commit().await {
                return ProblemDetails::internal_error(e.to_string()).into_response();
            }
            let _ = state
                .audit_log_store
                .append(AuditLog {
                    id: generate_id(),
                    tenant_id: String::new(),
                    actor_id: "mgmt".into(),
                    actor_type: IdentityType::ServiceAccount,
                    action: "role.deleted".into(),
                    target_type: "role".into(),
                    target_id: id,
                    metadata: json!({"name": role.name}),
                    created_at: Utc::now(),
                })
                .await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(_) => ProblemDetails::not_found("Role not found").into_response(),
        Err(e) => ProblemDetails::internal_error(e.to_string()).into_response(),
    }
}
