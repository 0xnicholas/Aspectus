use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_auth::password::PasswordHasher;

use crate::error::ProblemDetails;
use crate::AppState;

// ---- /authorize ----

#[derive(Deserialize)]
pub struct AuthorizeRequest {
    email: String,
    password: String,
    client_id: String,
    redirect_uri: String,
}

pub async fn authorize(
    State(state): State<AppState>,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    // v0.7: Validate redirect_uri against registered client
    let valid = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM oauth2_clients WHERE client_id = $1 AND $2 = ANY(redirect_uris))",
    )
    .bind(&req.client_id)
    .bind(&req.redirect_uri)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !valid {
        return ProblemDetails::validation_failed("Invalid client_id or redirect_uri", vec![]).into_response();
    }

    let user = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, tenant_id, password_hash FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, tenant_id, hash))) => {
            match PasswordHasher::verify(&req.password, &hash) {
                Ok(true) => (id, tenant_id),
                _ => return ProblemDetails::unauthorized("Invalid credentials", "/authorize").into_response(),
            }
        }
        _ => return ProblemDetails::unauthorized("Invalid credentials", "/authorize").into_response(),
    };

    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let code = hex::encode(Sha256::digest(raw));

    sqlx::query(
        "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&code).bind(&user.0).bind(&req.client_id)
    .bind(&req.redirect_uri).bind(Utc::now() + chrono::Duration::seconds(60))
    .execute(&state.pool).await.unwrap();

    (StatusCode::OK, Json(json!({"code": code, "redirect_uri": req.redirect_uri}))).into_response()
}

// ---- /token (extended with refresh_token) ----

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    client_id: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> impl IntoResponse {
    match req.grant_type.as_str() {
        "authorization_code" => {
            let code = match &req.code {
                Some(c) => c.clone(),
                None => return ProblemDetails::validation_failed("code required", vec![]).into_response(),
            };
            let row = sqlx::query_as::<_, (String, String, String)>(
                "UPDATE authorization_codes SET used = true \
                 WHERE code = $1 AND used = false AND expires_at > now() \
                 RETURNING user_id, client_id, redirect_uri",
            ).bind(&code).fetch_optional(&state.pool).await;
            let (user_id, client_id, _) = match row {
                Ok(Some(r)) => r,
                _ => return ProblemDetails::unauthorized("Invalid or expired code", "/token").into_response(),
            };
            let user = match sqlx::query_as::<_, (String,)>("SELECT tenant_id FROM users WHERE id = $1")
                .bind(&user_id).fetch_optional(&state.pool).await
            {
                Ok(Some((tid,))) => tid,
                _ => return ProblemDetails::internal_error("User not found").into_response(),
            };
            issue_tokens(&state, &user_id, &user, &client_id).await.into_response()
        }
        "refresh_token" => {
            let token = match &req.refresh_token {
                Some(t) => t.clone(),
                None => return ProblemDetails::validation_failed("refresh_token required", vec![]).into_response(),
            };
            let hash = hex::encode(Sha256::digest(token.as_bytes()));
            let row = sqlx::query_as::<_, (String, String, String)>(
                "UPDATE refresh_tokens SET revoked_at = now() \
                 WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() \
                 RETURNING user_id, client_id, token_hash",
            ).bind(&hash).fetch_optional(&state.pool).await;
            let (user_id, client_id, _) = match row {
                Ok(Some(r)) => r,
                _ => return ProblemDetails::unauthorized("Invalid or expired refresh_token", "/token").into_response(),
            };
            let user = match sqlx::query_as::<_, (String,)>("SELECT tenant_id FROM users WHERE id = $1")
                .bind(&user_id).fetch_optional(&state.pool).await
            {
                Ok(Some((tid,))) => tid,
                _ => return ProblemDetails::internal_error("User not found").into_response(),
            };
            issue_tokens(&state, &user_id, &user, &client_id).await.into_response()
        }
        _ => ProblemDetails::validation_failed("Unsupported grant_type", vec![]).into_response(),
    }
}

async fn issue_tokens(
    state: &AppState, user_id: &str, tenant_id: &str, client_id: &str,
) -> impl IntoResponse {
    let ttl: u64 = std::env::var("JWT_TTL_SECONDS").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(900);

    let project = client_id.parse().unwrap_or(aspectus_core::project::Project::Pandaria);

    // Expand scope from user roles
    let scopes = crate::scope_expander::ScopeExpander::expand(&state.pool, user_id).await;

    let access = match state.jwt_signer.sign(user_id, tenant_id, project, &scopes, ttl) {
        Ok(t) => t,
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    // Issue refresh token
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let refresh = format!("rt_{}", hex::encode(raw));
    let refresh_hash = hex::encode(Sha256::digest(refresh.as_bytes()));

    let _ = sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, client_id, expires_at) VALUES ($1, $2, $3, $4)",
    ).bind(&refresh_hash).bind(user_id).bind(client_id)
     .bind(Utc::now() + chrono::Duration::days(30))
     .execute(&state.pool).await;

    (StatusCode::OK, Json(json!({
        "access_token": access, "token_format": "jwt",
        "expires_in": ttl, "token_type": "Bearer",
        "refresh_token": refresh
    }))).into_response()
}

// ---- /clients (v0.7) ----

#[derive(Deserialize)]
pub struct CreateClientRequest {
    name: String,
    redirect_uris: Vec<String>,
}

pub async fn create_client(
    State(state): State<AppState>,
    Json(req): Json<CreateClientRequest>,
) -> impl IntoResponse {
    let id = format!("client_{}", crate::util::generate_id());
    sqlx::query(
        "INSERT INTO oauth2_clients (client_id, name, redirect_uris) VALUES ($1, $2, $3)",
    ).bind(&id).bind(&req.name).bind(&req.redirect_uris)
     .execute(&state.pool).await.unwrap();

    (StatusCode::CREATED, Json(json!({"client_id": id, "name": req.name}))).into_response()
}

pub async fn list_clients(State(state): State<AppState>) -> impl IntoResponse {
    let rows: Vec<(String, String, Vec<String>)> = sqlx::query_as(
        "SELECT client_id, name, redirect_uris FROM oauth2_clients ORDER BY created_at DESC",
    ).fetch_all(&state.pool).await.unwrap_or_default();

    let clients: Vec<serde_json::Value> = rows.into_iter().map(|(id, name, uris)| {
        json!({"client_id": id, "name": name, "redirect_uris": uris})
    }).collect();

    (StatusCode::OK, Json(clients)).into_response()
}
