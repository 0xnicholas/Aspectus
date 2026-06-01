use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_auth::password::PasswordHasher;

use crate::error::ProblemDetails;
use crate::AppState;

#[derive(Deserialize)]
pub struct AuthorizeRequest {
    email: String,
    password: String,
    client_id: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    code: Option<String>,
    client_id: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    // Find user by email within tenant (v0.6: search all tenants, v0.7: scope by client_id)
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

    // Generate authorization code
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let code = hex::encode(Sha256::digest(&raw));

    sqlx::query(
        "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&code)
    .bind(&user.0)
    .bind(&req.client_id)
    .bind(&req.redirect_uri)
    .bind(Utc::now() + chrono::Duration::seconds(60))
    .execute(&state.pool)
    .await
    .unwrap();

    (StatusCode::OK, Json(json!({
        "code": code,
        "redirect_uri": req.redirect_uri
    }))).into_response()
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
            )
            .bind(&code)
            .fetch_optional(&state.pool)
            .await;

            let (user_id, client_id, _redirect_uri) = match row {
                Ok(Some(r)) => r,
                _ => return ProblemDetails::unauthorized("Invalid or expired code", "/token").into_response(),
            };

            // Get user info
            let user = match sqlx::query_as::<_, (String,)>(
                "SELECT tenant_id FROM users WHERE id = $1",
            )
            .bind(&user_id)
            .fetch_optional(&state.pool)
            .await
            {
                Ok(Some((tid,))) => tid,
                _ => return ProblemDetails::internal_error("User not found").into_response(),
            };

            // Sign JWT
            let ttl: u64 = std::env::var("JWT_TTL_SECONDS").ok()
                .and_then(|s| s.parse().ok()).unwrap_or(900);

            match state.jwt_signer.sign(&user_id, &user, client_id.parse().unwrap_or(aspectus_core::project::Project::Pandaria), "", ttl) {
                Ok(token) => (StatusCode::OK, Json(json!({
                    "access_token": token,
                    "token_format": "jwt",
                    "expires_in": ttl,
                    "token_type": "Bearer"
                }))).into_response(),
                Err(e) => ProblemDetails::from(e).into_response(),
            }
        }
        _ => ProblemDetails::validation_failed("Unsupported grant_type", vec![]).into_response(),
    }
}
