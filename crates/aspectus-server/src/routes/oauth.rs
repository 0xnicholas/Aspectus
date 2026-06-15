use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_auth::password::PasswordHasher;
use aspectus_core::store::{
    AuthorizationCodeStore, RefreshTokenStore, OAuth2ClientStore,
};

use crate::error::ProblemDetails;
use crate::util::generate_id;
use crate::AppState;

// ---- /authorize ----

#[derive(Deserialize)]
pub struct AuthorizeRequest {
    email: String,
    password: String,
    client_id: String,
    redirect_uri: String,
    #[serde(default)]
    code_challenge: Option<String>,
    #[serde(default)]
    code_challenge_method: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    state: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    Json(req): Json<AuthorizeRequest>,
) -> impl IntoResponse {
    // Validate redirect_uri against registered client
    let valid = match state.oauth_client_store
        .validate_redirect_uri(&req.client_id, &req.redirect_uri)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "Failed to validate OAuth2 client");
            return ProblemDetails::internal_error("Authentication service temporarily unavailable").into_response();
        }
    };

    if !valid {
        return ProblemDetails::validation_failed("Invalid client_id or redirect_uri", vec![]).into_response();
    }

    // PKCE: if code_challenge is provided, method must be S256
    if req.code_challenge.is_some() && req.code_challenge_method.as_deref() != Some("S256") {
        return ProblemDetails::validation_failed(
            "code_challenge_method must be S256 when code_challenge is provided",
            vec![],
        ).into_response();
    }

    let (user_id, _tenant_id) = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, tenant_id, password_hash FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, tid, hash))) => {
            match PasswordHasher::verify(&req.password, &hash) {
                Ok(true) => (id, tid),
                _ => return ProblemDetails::unauthorized("Invalid credentials", "/authorize").into_response(),
            }
        }
        Ok(None) => return ProblemDetails::unauthorized("Invalid credentials", "/authorize").into_response(),
        Err(e) => {
            tracing::error!(error = %e, email = %req.email, "User lookup failed");
            return ProblemDetails::internal_error("Authentication service temporarily unavailable").into_response();
        }
    };

    // Generate authorization code
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let code = hex::encode(Sha256::digest(raw));

    let expires_at = Utc::now() + chrono::Duration::seconds(300);

    let _ = state.auth_code_store
        .create_code(&code, &user_id, &req.client_id, &req.redirect_uri, expires_at)
        .await;

    // Store code_challenge if PKCE is in use
    if let Some(ref challenge) = req.code_challenge {
        let _ = sqlx::query(
            "UPDATE authorization_codes SET code_challenge = $1 WHERE code = $2",
        )
        .bind(challenge)
        .bind(&code)
        .execute(&state.pool)
        .await;
    }

    (StatusCode::OK, Json(json!({"code": code, "redirect_uri": req.redirect_uri}))).into_response()
}

// ---- /token ----

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    code_verifier: Option<String>,
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

            let row = match state.auth_code_store.exchange_code(&code).await {
                Ok(Some(r)) => r,
                Ok(None) => return ProblemDetails::unauthorized("Invalid or expired code", "/token").into_response(),
                Err(e) => {
                    tracing::error!(error = %e, "Code exchange failed");
                    return ProblemDetails::internal_error("Token service temporarily unavailable").into_response();
                }
            };
            let (user_id, client_id, _redirect_uri) = row;

            // PKCE: verify code_verifier if code_challenge was stored
            if let Some(ref verifier) = req.code_verifier {
                let challenge: Option<String> = sqlx::query_scalar(
                    "SELECT code_challenge FROM authorization_codes WHERE code = $1",
                )
                .bind(&code)
                .fetch_optional(&state.pool)
                .await
                .unwrap_or_default()
                .flatten();

                if let Some(expected_challenge) = challenge {
                    let actual_challenge = hex::encode(Sha256::digest(verifier.as_bytes()));
                    if actual_challenge != expected_challenge {
                        return ProblemDetails::unauthorized("Invalid code_verifier", "/token").into_response();
                    }
                }
            }

            let tenant_id = match sqlx::query_as::<_, (String,)>(
                "SELECT tenant_id FROM users WHERE id = $1",
            )
            .bind(&user_id)
            .fetch_optional(&state.pool)
            .await
            {
                Ok(Some((tid,))) => tid,
                Ok(None) => return ProblemDetails::internal_error("User not found").into_response(),
                Err(e) => {
                    tracing::error!(error = %e, user_id = %user_id, "User lookup failed");
                    return ProblemDetails::internal_error("Token service temporarily unavailable").into_response();
                }
            };

            issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
        }
        "refresh_token" => {
            let token = match &req.refresh_token {
                Some(t) => t.clone(),
                None => return ProblemDetails::validation_failed("refresh_token required", vec![]).into_response(),
            };
            let hash = hex::encode(Sha256::digest(token.as_bytes()));

            match state.refresh_token_store.rotate(&hash).await {
                Ok(Some((user_id, client_id, _))) => {
                    let tenant_id = match sqlx::query_as::<_, (String,)>(
                        "SELECT tenant_id FROM users WHERE id = $1",
                    )
                    .bind(&user_id)
                    .fetch_optional(&state.pool)
                    .await
                    {
                        Ok(Some((tid,))) => tid,
                        _ => return ProblemDetails::internal_error("User not found").into_response(),
                    };
                    issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
                }
                Ok(None) => {
                    // Token not found or already revoked — check for replay attack
                    if let Ok(Some((user_id, is_revoked))) =
                        state.refresh_token_store.find_by_hash_any(&hash).await
                        && is_revoked
                    {
                        // Replay detected: revoke all tokens for this user
                        let count = state.refresh_token_store
                            .revoke_all_for_user(&user_id)
                            .await
                            .unwrap_or(0);
                        tracing::warn!(
                            user_id = %user_id,
                            revoked = count,
                            "Refresh token replay detected — revoked all user tokens"
                        );
                    }
                    ProblemDetails::unauthorized("Invalid or expired refresh_token", "/token").into_response()
                }
                Err(e) => {
                    tracing::error!(error = %e, "Refresh token rotation failed");
                    ProblemDetails::internal_error("Token service temporarily unavailable").into_response()
                }
            }
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
    let scopes = crate::scope_expander::ScopeExpander::expand(&state.pool, user_id, Some(&state.scope_cache)).await;

    let access = match state.jwt_signer.sign(user_id, tenant_id, project, &scopes, ttl) {
        Ok(t) => t,
        Err(e) => return ProblemDetails::from(e).into_response(),
    };

    // Issue refresh token
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let refresh = format!("rt_{}", hex::encode(raw));
    let refresh_hash = hex::encode(Sha256::digest(refresh.as_bytes()));

    let _ = state.refresh_token_store
        .create(
            &refresh_hash,
            user_id,
            client_id,
            Utc::now() + chrono::Duration::days(30),
        )
        .await;

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
    let id = format!("client_{}", generate_id());

    match state.oauth_client_store
        .create(&id, &req.name, &req.redirect_uris)
        .await
    {
        Ok(()) => (StatusCode::CREATED, Json(json!({"client_id": id, "name": req.name}))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to create OAuth2 client");
            ProblemDetails::internal_error("Failed to create client").into_response()
        }
    }
}

pub async fn list_clients(State(state): State<AppState>) -> impl IntoResponse {
    match state.oauth_client_store.list().await {
        Ok(rows) => {
            let clients: Vec<serde_json::Value> = rows.into_iter().map(|(id, name, uris)| {
                json!({"client_id": id, "name": name, "redirect_uris": uris})
            }).collect();
            (StatusCode::OK, Json(clients)).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to list OAuth2 clients");
            (StatusCode::OK, Json(Vec::<serde_json::Value>::new())).into_response()
        }
    }
}
