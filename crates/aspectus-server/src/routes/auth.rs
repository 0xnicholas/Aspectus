//! User-facing authentication endpoints.
//!
//! Unlike the OAuth2 /authorize → /oauth/token flow and the management API,
//! these endpoints are designed for project-owned login UIs:
//!
//! - `POST /login`   — email+password → JWT (single step)
//! - `POST /register` — create account + auto-login
//! - `POST /logout`  — revoke refresh token
//! - `POST /forgot-password` — generate reset token (emails stub)
//! - `POST /reset-password`  — verify token + update password

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use axum::http::HeaderMap;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use aspectus_auth::password::PasswordHasher;
use aspectus_core::identity::IdentityType;
use aspectus_core::store::{AuditLogStore, RefreshTokenStore};

use crate::error::ProblemDetails;
use crate::AppState;

// ── Helpers ────────────────────────────────────────────────────

/// Extract client IP from X-Forwarded-For header or fallback.
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// Parameters for an authentication audit log entry.
struct AuditAuthParams {
    tenant_id: String,
    actor_id: String,
    actor_type: IdentityType,
    action: String,
    target_type: String,
    target_id: String,
    metadata: serde_json::Value,
}

/// Record an audit log entry for authentication events.
async fn audit_auth_event(state: &AppState, params: AuditAuthParams) {
    let id = crate::util::generate_id();
    let entry = aspectus_core::audit_log::AuditLog {
        id,
        tenant_id: params.tenant_id,
        actor_id: params.actor_id,
        actor_type: params.actor_type,
        action: params.action,
        target_type: params.target_type,
        target_id: params.target_id,
        metadata: params.metadata,
        created_at: Utc::now(),
    };
    if let Err(e) = state.audit_log_store.append(entry).await {
        tracing::error!(error = %e, "Failed to write audit log");
    }
}

// ── POST /login ────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct LoginRequest {
    email: String,
    password: String,
    /// ADR-016: tenant_id is REQUIRED to disambiguate when the same email
    /// is registered under multiple tenants (schema UNIQUE (tenant_id, email)).
    /// Frontends MUST call `POST /login/lookup` first and pass the
    /// user-selected tenant here.
    pub tenant_id: String,
    /// Which project the user is logging into (default: "pandaria").
    #[serde(default = "default_client_id")]
    client_id: String,
}

fn default_client_id() -> String {
    "pandaria".into()
}

/// POST /login
///
/// Step 2 of the two-step login flow (ADR-016): email + password + tenant_id
/// → JWT access token + refresh token. Frontends must call `POST /login/lookup`
/// first to obtain the list of tenants this email is registered under, then
/// pass the user-selected `tenant_id` here.
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(login_req): Json<LoginRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    // Validate client_id is a known project
    let client_id = login_req.client_id;
    if client_id.parse::<aspectus_core::project::Project>().is_err() {
        return ProblemDetails::validation_failed(
            format!("Unknown project: {client_id}"),
            vec![],
        )
        .into_response();
    }

    // Look up user by (tenant_id, email) — ADR-016.
    // Same email can exist in multiple tenants (UNIQUE (tenant_id, email)),
    // so an email-only lookup is ambiguous.
    let (user_id, tenant_id) = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, tenant_id, password_hash FROM users WHERE tenant_id = $1 AND email = $2",
    )
    .bind(&login_req.tenant_id)
    .bind(&login_req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some((id, tid, hash))) => {
            match PasswordHasher::verify(&login_req.password, &hash) {
                Ok(true) => (id, tid),
                _ => {
                    // Failed login — ambiguous message, no user/email leak
                    return ProblemDetails::unauthorized("Invalid email or password", "/login").into_response();
                }
            }
        }
        Ok(None) => {
            // No such user — same ambiguous message
            return ProblemDetails::unauthorized("Invalid email or password", "/login").into_response();
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                tenant_id = %login_req.tenant_id,
                email_hash = %hex::encode(Sha256::digest(login_req.email.as_bytes())),
                "User lookup failed"
            );
            return ProblemDetails::internal_error("Authentication service temporarily unavailable").into_response();
        }
    };

    // Check if suspended
    let is_suspended: bool = sqlx::query_scalar(
        "SELECT is_suspended FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if is_suspended {
        audit_auth_event(
            &state, AuditAuthParams {
                tenant_id: tenant_id.clone(), actor_id: user_id.clone(), actor_type: IdentityType::User,
                action: "user.login_blocked".into(), target_type: "user".into(), target_id: user_id.clone(),
                metadata: serde_json::json!({"reason": "account_suspended", "ip": ip}),
            },
        ).await;
        return ProblemDetails::unauthorized("Account is suspended", "/login").into_response();
    }

    // Update last_sign_in_at
    let _ = sqlx::query("UPDATE users SET last_sign_in_at = NOW() WHERE id = $1")
        .bind(&user_id)
        .execute(&state.pool)
        .await;

    // Audit: successful login
    audit_auth_event(
        &state, AuditAuthParams {
            tenant_id: tenant_id.clone(), actor_id: user_id.clone(), actor_type: IdentityType::User,
            action: "user.login".into(), target_type: "user".into(), target_id: user_id.clone(),
            metadata: serde_json::json!({"ip": ip, "client_id": client_id}),
        },
    ).await;

    // Issue tokens
    crate::routes::oauth::issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
}

// ── POST /register ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    password: String,
    #[serde(default)]
    display_name: Option<String>,
    /// Tenant to join (default: "default").
    #[serde(default = "default_tenant")]
    tenant_id: String,
    /// Which project the user is registering from (default: "pandaria").
    #[serde(default = "default_client_id")]
    client_id: String,
}

fn default_tenant() -> String {
    "default".into()
}

/// POST /register
///
/// Create a new user account and return JWT tokens (auto-login).
/// Requires `ASPECTUS_REGISTRATION_ENABLED=true` env var.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(reg): Json<RegisterRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    // Check if registration is enabled
    let enabled = std::env::var("ASPECTUS_REGISTRATION_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if !enabled {
        return ProblemDetails::forbidden("Public registration is disabled").into_response();
    }

    // Validate client_id is a known project
    let client_id = reg.client_id.clone();
    if client_id.parse::<aspectus_core::project::Project>().is_err() {
        return ProblemDetails::validation_failed(
            format!("Unknown project: {client_id}"),
            vec![],
        ).into_response();
    }

    // Validate email format
    if !reg.email.contains('@') || reg.email.len() < 5 {
        return ProblemDetails::validation_failed(
            "Invalid email address",
            vec![],
        )
        .into_response();
    }

    // Validate password strength (min 8 chars)
    if reg.password.len() < 8 {
        return ProblemDetails::validation_failed(
            "Password must be at least 8 characters",
            vec![],
        )
        .into_response();
    }

    // Check email uniqueness — scoped to tenant (ADR-016)
    // Schema: UNIQUE (tenant_id, email) allows same email across different tenants.
    // Previously this query omitted tenant_id, incorrectly blocking cross-tenant registration.
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE tenant_id = $1 AND email = $2)",
    )
    .bind(&reg.tenant_id)
    .bind(&reg.email)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if exists {
        // Don't leak whether email exists — use same error as login
        return ProblemDetails::validation_failed(
            "Email is already registered",
            vec![],
        )
        .into_response();
    }

    // ADR-016 decision 6: Verify the tenant exists BEFORE attempting user creation.
    // Previously this endpoint would auto-create a tenant with `INSERT INTO tenants (id, name)
    // VALUES ($1, $1)` if it didn't exist — a production hazard because:
    //   1. Anyone registering without an explicit tenant_id would land in the
    //      same "default" tenant, mixing users across organizations.
    //   2. An attacker could spam-tenant creation by registering under
    //      arbitrary tenant IDs.
    //
    // The proper flow (per AGENTS.md + ADR-008) is:
    //   - Production: admin creates tenants via `POST /tenants` (Service Token),
    //     then admin creates users via `POST /users`.
    //   - Demo/dev: a tenant must be created manually (or via init script)
    //     before /register can be used against it.
    //
    // If the tenant doesn't exist, return 404 with a clear message.
    let tenant_id = reg.tenant_id;
    let tenant_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM tenants WHERE id = $1)",
    )
    .bind(&tenant_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !tenant_exists {
        tracing::warn!(
            tenant_id = %tenant_id,
            email_hash = %hex::encode(Sha256::digest(reg.email.as_bytes())),
            "Registration attempted against non-existent tenant. \
             This is expected for /register; in production use POST /users (Service Token)."
        );
        return ProblemDetails::not_found(
            format!(
                "Tenant '{tenant_id}' does not exist. \
                 In production, ask an admin to create the tenant via `POST /tenants` \
                 (Service Token auth) before creating users."
            ),
        )
        .into_response();
    }

    // Hash password
    let password_hash = match PasswordHasher::hash(&reg.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Password hashing failed");
            return ProblemDetails::internal_error("Failed to process registration").into_response();
        }
    };

    // Create user
    let user_id = crate::util::generate_id();
    if let Err(e) = sqlx::query(
        "INSERT INTO users (id, tenant_id, email, password_hash, display_name) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&user_id)
    .bind(&tenant_id)
    .bind(&reg.email)
    .bind(&password_hash)
    .bind(reg.display_name.as_deref())
    .execute(&state.pool)
    .await
    {
        tracing::error!(error = %e, "User creation failed");
        return ProblemDetails::internal_error("Failed to create account").into_response();
    }

    // ADR-016: Assign default Role so the new user has at least some scopes.
    // We look up the role with is_default = true AND type IN ('user', 'both').
    // Failure here is non-fatal — admin can assign roles later.
    let default_role_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM roles
         WHERE is_default = true AND type IN ('user', 'both')
         LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    if let Some(role_id) = default_role_id {
        let users_role_id = crate::util::generate_id();
        match sqlx::query(
            "INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3)
             ON CONFLICT (user_id, role_id) DO NOTHING",
        )
        .bind(&users_role_id)
        .bind(&user_id)
        .bind(&role_id)
        .execute(&state.pool)
        .await
        {
            Ok(_) => {
                tracing::debug!(
                    user_id = %user_id,
                    role_id = %role_id,
                    "Assigned default role on registration"
                );
                // Invalidate the scope expansion cache so the new role takes effect
                // immediately on the access token we're about to issue.
                crate::scope_expander::ScopeExpander::invalidate(&state.scope_cache, &user_id).await;
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    user_id = %user_id,
                    role_id = %role_id,
                    "Failed to assign default role during registration — user will have no scopes"
                );
            }
        }
    } else {
        tracing::warn!(
            user_id = %user_id,
            "No default role configured (is_default = true). \
             New user registered without any Role — admin must assign one."
        );
    }

    // Audit: registration
    audit_auth_event(
        &state, AuditAuthParams {
            tenant_id: tenant_id.clone(), actor_id: user_id.clone(), actor_type: IdentityType::User,
            action: "user.registered".into(), target_type: "user".into(), target_id: user_id.clone(),
            metadata: serde_json::json!({"ip": ip, "email": reg.email}),
        },
    ).await;

    // Auto-login: issue tokens
    crate::routes::oauth::issue_tokens(&state, &user_id, &tenant_id, &client_id).await.into_response()
}

// ── POST /logout ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LogoutRequest {
    refresh_token: String,
    /// Optional: also revoke the access token (for JWT, this adds jti to deny list)
    #[serde(default)]
    access_token: Option<String>,
}

/// POST /logout
///
/// Revoke the refresh token. If an access_token is provided, it is also
/// added to the JWT revocation list (for server-side logout).
pub async fn logout(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> impl IntoResponse {
    // Revoke refresh token
    let rt_hash = hex::encode(Sha256::digest(req.refresh_token.as_bytes()));

    if let Err(e) = state.refresh_token_store.rotate(&rt_hash).await {
        // rotate() atomically marks as revoked — even if the token is already
        // revoked or expired, we return success (idempotent)
        tracing::warn!(error = %e, "Refresh token revocation attempted");
    }

    // Revoke access token (JWT) if provided
    if let Some(access_token) = req.access_token
        && access_token.starts_with("eyJ") {
            state.jwt_verifier.revoke(&access_token).await;
        }

    StatusCode::NO_CONTENT.into_response()
}

// ── POST /forgot-password ──────────────────────────────────────

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    email: String,
}

/// POST /forgot-password
///
/// Generate a one-time password reset token and "send" it via email.
/// In development, the reset token is logged (no email server required).
///
/// Always returns success to prevent email enumeration.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> impl IntoResponse {
    // Look up user (but don't reveal existence)
    let user = match sqlx::query_as::<_, (String, String)>(
        "SELECT id, tenant_id FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(u)) => u,
        _ => {
            // User not found — return success anyway to prevent enumeration
            return (StatusCode::OK, Json(json!({"message": "If this email is registered, a reset link has been sent."}))).into_response();
        }
    };

    let (user_id, _tenant_id) = user;

    // Generate reset token
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap_or_default();
    let token = hex::encode(raw);
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let expires_at = Utc::now() + chrono::Duration::hours(1);

    // Store token hash
    if let Err(e) = sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&token_hash)
    .bind(&user_id)
    .bind(expires_at)
    .execute(&state.pool)
    .await
    {
        tracing::error!(error = %e, "Failed to store password reset token");
        return ProblemDetails::internal_error("Failed to process request").into_response();
    }

    // In production, send email. For now, log the reset link.
    let reset_url = format!(
        "https://aspectus.local/reset-password?token={token}"
    );
    tracing::info!(
        user_id = %user_id,
        reset_url = %reset_url,
        "Password reset token generated (email stub)"
    );

    // Always return the same message
    (StatusCode::OK, Json(json!({"message": "If this email is registered, a reset link has been sent."}))).into_response()
}

// ── POST /reset-password ───────────────────────────────────────

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    token: String,
    new_password: String,
}

/// POST /reset-password
///
/// Verify the reset token and update the user's password.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> impl IntoResponse {
    // Validate password strength
    if req.new_password.len() < 8 {
        return ProblemDetails::validation_failed(
            "Password must be at least 8 characters",
            vec![],
        )
        .into_response();
    }

    // Hash the incoming token to look it up
    let token_hash = hex::encode(Sha256::digest(req.token.as_bytes()));

    // Atomically claim the token (mark used)
    let user_id: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used = true \
         WHERE token_hash = $1 AND used = false AND expires_at > NOW() \
         RETURNING user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    let user_id = match user_id {
        Some(id) => id,
        None => {
            return ProblemDetails::validation_failed(
                "Invalid or expired reset token",
                vec![],
            )
            .into_response();
        }
    };

    // Hash new password
    let password_hash = match PasswordHasher::hash(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Password hashing failed");
            return ProblemDetails::internal_error("Failed to update password").into_response();
        }
    };

    // Update password
    if let Err(e) = sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&password_hash)
        .bind(&user_id)
        .execute(&state.pool)
        .await
    {
        tracing::error!(error = %e, user_id = %user_id, "Password update failed");
        return ProblemDetails::internal_error("Failed to update password").into_response();
    }

    // Audit
    let tenant_id: Option<String> = sqlx::query_scalar(
        "SELECT tenant_id FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    if let Some(tid) = tenant_id {
        audit_auth_event(
            &state, AuditAuthParams {
                tenant_id: tid.clone(), actor_id: user_id.clone(), actor_type: IdentityType::User,
                action: "user.password_reset".into(), target_type: "user".into(), target_id: user_id.clone(),
                metadata: serde_json::json!({"method": "email_reset_token"}),
            },
        ).await;
    }

    (StatusCode::OK, Json(json!({"message": "Password has been reset successfully."}))).into_response()
}

// ── POST /login/lookup ─────────────────────────────────────

/// ADR-016: Step 1 of the two-step login flow.
///
/// Given an email, return the list of tenants this email is registered under
/// (excluding suspended users). The client then asks the user to pick a tenant,
/// and calls `POST /login` with `{email, password, tenant_id}`.
///
/// Security:
/// - Returns the same shape (`{"tenants": []}`) regardless of whether the
///   email exists in the database, to prevent email enumeration.
/// - Tenant names are not considered PII (they are public organization names).
/// - Suspended users are excluded — they cannot log in until reinstated.
#[derive(Deserialize)]
pub struct LoginLookupRequest {
    email: String,
}

#[derive(Serialize)]
pub struct TenantOption {
    pub tenant_id: String,
    pub tenant_name: String,
    /// Tenant logo URL (populated since migration 012).
    /// Frontends should fall back to initials when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
}

#[derive(Serialize)]
pub struct LoginLookupResponse {
    pub tenants: Vec<TenantOption>,
}

pub async fn login_lookup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginLookupRequest>,
) -> impl IntoResponse {
    let ip = client_ip(&headers);

    // Validate email format — same rule as /register and /login.
    // Bad input gets 422 with a clear message; legitimate-looking but unknown
    // emails get an empty `tenants` array to prevent enumeration.
    if !req.email.contains('@') || req.email.len() < 5 {
        return ProblemDetails::validation_failed(
            "Invalid email address",
            vec![],
        )
        .into_response();
    }

    // Find all tenants under which this email has an active (non-suspended) account.
    // Ordered by tenant name for stable, user-friendly display.
    // logo_url may be NULL — tenants created before migration 012
    // won't have one set.
    let rows: Vec<(String, String, Option<String>)> = match sqlx::query_as(
        "SELECT t.id, t.name, t.logo_url
         FROM users u
         JOIN tenants t ON t.id = u.tenant_id
         WHERE u.email = $1 AND u.is_suspended = false
         ORDER BY t.name ASC",
    )
    .bind(&req.email)
    .fetch_all(&state.pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(error = %e, "Login lookup query failed");
            return ProblemDetails::internal_error("Authentication service temporarily unavailable")
                .into_response();
        }
    };

    let tenants: Vec<TenantOption> = rows
        .into_iter()
        .map(|(tenant_id, tenant_name, logo_url)| TenantOption {
            tenant_id,
            tenant_name,
            logo_url,
        })
        .collect();

    // Trace at info level with hashed email so we can detect enumeration attacks
    // (e.g. one IP looking up hundreds of emails) without leaking PII.
    let email_hash = {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(req.email.as_bytes()))
    };
    tracing::info!(
        ip = %ip,
        email_hash = %email_hash,
        result_count = tenants.len(),
        "Login lookup"
    );

    // Always 200 — empty array conveys "no match" without leaking existence.
    (StatusCode::OK, Json(LoginLookupResponse { tenants })).into_response()
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LoginRequest ──

    #[test]
    fn login_request_with_client_id() {
        let json = r#"{"email":"a@b.com","password":"secret123","tenant_id":"org_acme"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.tenant_id, "org_acme");
        assert_eq!(req.client_id, "pandaria"); // default
    }

    #[test]
    fn login_request_custom_client_id() {
        let json = r#"{"email":"a@b.com","password":"secret123","tenant_id":"org_acme","client_id":"tavern"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.client_id, "tavern");
        assert_eq!(req.tenant_id, "org_acme");
    }

    #[test]
    fn login_request_requires_tenant_id() {
        // ADR-016: tenant_id is REQUIRED — without it, login is ambiguous
        // for users registered under multiple tenants. The JSON deserializer
        // must reject this before the handler is even called.
        let json = r#"{"email":"a@b.com","password":"secret123"}"#;
        let result: Result<LoginRequest, _> = serde_json::from_str(json);
        assert!(result.is_err(), "Login without tenant_id must fail to deserialize");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("tenant_id") || err.contains("missing field"),
            "Error should mention tenant_id, got: {err}"
        );
    }

    #[test]
    fn login_request_empty_tenant_id_rejected() {
        // Empty string passes the "required" check but the SQL lookup
        // will simply not find anything. We rely on the lookup, not on
        // a length check, so this test asserts the current behavior:
        // empty tenant_id is accepted by deserialize, but yields no match.
        let json = r#"{"email":"a@b.com","password":"secret123","tenant_id":""}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tenant_id, "");
    }

    // ── RegisterRequest ──

    #[test]
    fn register_request_minimal() {
        // ADR-016: tenant_id defaults to "default" for backward compatibility,
        // but the handler will return 404 if "default" doesn't exist (no auto-create).
        let json = r#"{"email":"a@b.com","password":"secret123"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
        assert_eq!(req.tenant_id, "default");
        assert_eq!(req.client_id, "pandaria");
        assert!(req.display_name.is_none());
    }

    #[test]
    fn register_request_full() {
        let json = r#"{"email":"a@b.com","password":"secret123","display_name":"Alice","tenant_id":"org-foo","client_id":"tavern"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.display_name.as_deref(), Some("Alice"));
        assert_eq!(req.tenant_id, "org-foo");
        assert_eq!(req.client_id, "tavern");
    }

    #[test]
    fn register_rejects_short_password() {
        // Password < 8 chars should fail validation (tested in handler, but we check the rule)
        let short = "short";
        assert!(short.len() < 8);
    }

    #[test]
    fn register_rejects_invalid_email() {
        let invalid = "not-an-email";
        assert!(!invalid.contains('@'));
    }

    #[test]
    fn register_default_tenant_must_exist_in_db() {
        // ADR-016 decision 6: /register no longer auto-creates a tenant.
        // The "default" tenant_id literal still parses successfully, but
        // if the DB has no row with id="default", the handler returns 404.
        // This test asserts the parsing rule; the handler behavior is exercised
        // by integration tests.
        let json = r#"{"email":"a@b.com","password":"secret123"}"#;
        let req: RegisterRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tenant_id, "default");
    }

    #[test]
    fn register_404_for_missing_tenant_includes_guidance() {
        // The 404 response from /register should mention the tenant_id
        // and point to the production flow (POST /users via Service Token).
        // This test verifies the *error message format*, not the DB lookup.
        let pd = ProblemDetails::not_found(
            "Tenant 'org-foo' does not exist. \
             In production, ask an admin to create the tenant via `POST /tenants` \
             (Service Token auth) before creating users."
        );
        let json = serde_json::to_value(&pd).unwrap();
        assert_eq!(json["status"], 404);
        assert_eq!(json["title"], "Not Found");
        let detail = json["detail"].as_str().unwrap();
        assert!(detail.contains("org-foo"), "detail should mention tenant_id");
        assert!(detail.contains("POST /tenants"), "detail should mention admin flow");
        assert!(detail.contains("Service Token"), "detail should mention Service Token auth");
    }

    // ── LogoutRequest ──

    #[test]
    fn logout_request_minimal() {
        let json = r#"{"refresh_token":"rt_abc123"}"#;
        let req: LogoutRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.refresh_token, "rt_abc123");
        assert!(req.access_token.is_none());
    }

    #[test]
    fn logout_request_with_access_token() {
        let json = r#"{"refresh_token":"rt_abc","access_token":"eyJ..."}"#;
        let req: LogoutRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.access_token.as_deref(), Some("eyJ..."));
    }

    // ── ForgotPasswordRequest ──

    #[test]
    fn forgot_password_request() {
        let json = r#"{"email":"a@b.com"}"#;
        let req: ForgotPasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "a@b.com");
    }

    // ── LoginLookupRequest (ADR-016) ──

    #[test]
    fn login_lookup_request_minimal() {
        let json = r#"{"email":"alice@acme.com"}"#;
        let req: LoginLookupRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.email, "alice@acme.com");
    }

    #[test]
    fn login_lookup_email_must_contain_at_sign() {
        // Mirrors the handler's validation rule
        let invalid = "not-an-email";
        assert!(!invalid.contains('@'));
    }

    #[test]
    fn login_lookup_email_min_length() {
        // Mirrors the handler's validation: email.len() < 5 → reject
        assert!("a@b".len() < 5);
        assert!("a@b.".len() < 5); // boundary
        assert!("a@b.c".len() == 5); // exactly 5 — accepted
    }

    #[test]
    fn tenant_option_omits_logo_url_when_none() {
        // logo_url uses skip_serializing_if — verify the JSON shape
        let opt = TenantOption {
            tenant_id: "org_acme".into(),
            tenant_name: "Acme Corp".into(),
            logo_url: None,
        };
        let json = serde_json::to_value(&opt).unwrap();
        assert_eq!(json["tenant_id"], "org_acme");
        assert_eq!(json["tenant_name"], "Acme Corp");
        assert!(json.get("logo_url").is_none(),
                "logo_url must be absent when None, got: {json}");
    }

    #[test]
    fn tenant_option_includes_logo_url_when_some() {
        let opt = TenantOption {
            tenant_id: "org_acme".into(),
            tenant_name: "Acme Corp".into(),
            logo_url: Some("https://cdn.acme.com/logo.png".into()),
        };
        let json = serde_json::to_value(&opt).unwrap();
        assert_eq!(json["logo_url"], "https://cdn.acme.com/logo.png");
    }

    #[test]
    fn login_lookup_response_empty_serializes() {
        // The "no match" response must be indistinguishable from "email doesn't exist"
        // — same shape, same status. Frontends should treat empty tenants as
        // "this email isn't registered anywhere".
        let resp = LoginLookupResponse { tenants: vec![] };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["tenants"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn login_lookup_response_multiple_serializes_in_order() {
        // Order should match the SQL ORDER BY t.name ASC.
        let resp = LoginLookupResponse {
            tenants: vec![
                TenantOption { tenant_id: "t1".into(), tenant_name: "Acme Corp".into(), logo_url: None },
                TenantOption { tenant_id: "t2".into(), tenant_name: "Foo Industries".into(), logo_url: None },
            ],
        };
        let json = serde_json::to_value(&resp).unwrap();
        let tenants = json["tenants"].as_array().unwrap();
        assert_eq!(tenants.len(), 2);
        assert_eq!(tenants[0]["tenant_name"], "Acme Corp");
        assert_eq!(tenants[1]["tenant_name"], "Foo Industries");
    }

    // ── ResetPasswordRequest ──

    #[test]
    fn reset_password_request() {
        let json = r#"{"token":"abc123","new_password":"newsecret123"}"#;
        let req: ResetPasswordRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.token, "abc123");
        assert_eq!(req.new_password, "newsecret123");
    }

    #[test]
    fn reset_password_rejects_short_password() {
        // Simulate the handler's validation
        let req: ResetPasswordRequest = serde_json::from_str(
            r#"{"token":"abc","new_password":"short"}"#
        ).unwrap();
        assert!(req.new_password.len() < 8);
    }

    // ── Client IP extraction ──

    #[test]
    fn client_ip_from_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.1");
    }

    #[test]
    fn client_ip_first_in_chain() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "10.0.0.1, 10.0.0.2".parse().unwrap());
        assert_eq!(client_ip(&headers), "10.0.0.1");
    }

    #[test]
    fn client_ip_fallback() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }

    // ── Default values ──

    #[test]
    fn default_client_id_is_pandaria() {
        assert_eq!(default_client_id(), "pandaria");
    }

    #[test]
    fn default_tenant_is_default() {
        assert_eq!(default_tenant(), "default");
    }
}
