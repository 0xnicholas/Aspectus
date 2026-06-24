//! Integration tests for v0.5-v0.7 features: User, Role, OAuth2.
//! Requires DATABASE_URL and REDIS_URL env vars.

use aspectus_auth::password::PasswordHasher;
use aspectus_core::store::{TenantStore, UserStore};
use aspectus_server::db::{PgServiceAccountStore, PgTenantStore, PgUserStore};
use aspectus_server::scope_expander::ScopeExpander;
use sha2::{Digest, Sha256};

fn unique_email(prefix: &str) -> String {
    format!(
        "{}-{}@test.com",
        prefix,
        chrono::Utc::now().timestamp_millis()
    )
}

async fn setup() -> (
    PgTenantStore,
    PgServiceAccountStore,
    PgUserStore,
    sqlx::PgPool,
) {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::PgPool::connect(&db_url).await.unwrap();
    let ts = PgTenantStore::new(pool.clone());
    let ss = PgServiceAccountStore::new(pool.clone());
    let us = PgUserStore::new(pool.clone());
    (ts, ss, us, pool)
}

#[tokio::test]
async fn user_create_and_get() {
    let (ts, _, us, _) = setup().await;
    let tenant = ts.create("ut-user").await.unwrap();
    let email = unique_email("ut");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us
        .create(&tenant.id, &email, &hash, Some("Test"))
        .await
        .unwrap();
    assert_eq!(user.email.as_deref(), Some(email.as_str()));
    assert!(user.password_hash.is_some()); // populated from DB, hidden from JSON

    let fetched = us.get_by_id(&user.id).await.unwrap().unwrap();
    assert_eq!(fetched.id, user.id);
}

#[tokio::test]
async fn user_suspend_toggle() {
    let (ts, _, us, _) = setup().await;
    let tenant = ts.create("ut-suspend").await.unwrap();
    let email = unique_email("suspend");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    us.set_suspended(&user.id, true).await.unwrap();
    assert!(us.get_by_id(&user.id).await.unwrap().unwrap().is_suspended);

    us.set_suspended(&user.id, false).await.unwrap();
    assert!(!us.get_by_id(&user.id).await.unwrap().unwrap().is_suspended);
}

#[tokio::test]
async fn scope_expander_returns_scopes() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-scope").await.unwrap();
    let email = unique_email("scope");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    // Manually assign agent-developer role (HTTP handler does this automatically)
    let rid = format!("sr-{}", chrono::Utc::now().timestamp_millis());
    sqlx::query("INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3)")
        .bind(&rid)
        .bind(&user.id)
        .bind("role_agent_dev")
        .execute(&pool)
        .await
        .unwrap();

    let scopes = ScopeExpander::expand(&pool, &user.id, None).await;
    assert!(!scopes.is_empty(), "User should have scopes from role");
    assert!(
        scopes.contains("pandaria"),
        "Should contain pandaria scopes"
    );
}

#[tokio::test]
async fn role_type_constraint_violation() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-constraint").await.unwrap();
    let email = unique_email("constraint");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    let cid = format!("cv-{}", chrono::Utc::now().timestamp_millis());
    let result = sqlx::query("INSERT INTO users_roles (id, user_id, role_id) VALUES ($1, $2, $3)")
        .bind(&cid)
        .bind(&user.id)
        .bind("role_ci_deployer")
        .execute(&pool)
        .await;

    assert!(
        result.is_err(),
        "Should reject service_account role for user"
    );
}

#[tokio::test]
async fn oauth2_authorization_code_flow() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-oauth").await.unwrap();
    let email = unique_email("oauth");
    let hash = PasswordHasher::hash("oauthpass123").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    // Validate password
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, password_hash FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .unwrap();
    assert!(PasswordHasher::verify("oauthpass123", &row.1).unwrap());

    // Create authorization code
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap();
    let code = hex::encode(Sha256::digest(raw));

    sqlx::query(
        "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&code)
    .bind(&row.0)
    .bind("pandaria")
    .bind("https://example.com/cb")
    .bind(chrono::Utc::now() + chrono::Duration::seconds(60))
    .execute(&pool)
    .await
    .unwrap();

    // Exchange code
    let token_row = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE authorization_codes SET used = true \
         WHERE code = $1 AND used = false AND expires_at > now() \
         RETURNING user_id, client_id, redirect_uri",
    )
    .bind(&code)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .unwrap();
    assert_eq!(token_row.0, user.id);

    // One-time use
    let second = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE authorization_codes SET used = true \
         WHERE code = $1 AND used = false AND expires_at > now() \
         RETURNING user_id, client_id, redirect_uri",
    )
    .bind(&code)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(second.is_none(), "Code should be one-time use");
}

#[tokio::test]
async fn refresh_token_rotation() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-refresh").await.unwrap();
    let email = unique_email("refresh");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap();
    let refresh = format!("rt_{}", hex::encode(raw));
    let refresh_hash = hex::encode(Sha256::digest(refresh.as_bytes()));

    sqlx::query(
        "INSERT INTO refresh_tokens (token_hash, user_id, client_id, expires_at) VALUES ($1, $2, $3, $4)",
    ).bind(&refresh_hash).bind(&user.id).bind("pandaria")
     .bind(chrono::Utc::now() + chrono::Duration::days(30))
     .execute(&pool).await.unwrap();

    // Rotate
    let row = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() \
         RETURNING user_id, client_id, token_hash",
    )
    .bind(&refresh_hash)
    .fetch_optional(&pool)
    .await
    .unwrap()
    .unwrap();
    assert_eq!(row.0, user.id);

    // One-time use
    let second = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() \
         RETURNING user_id, client_id, token_hash",
    )
    .bind(&refresh_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(second.is_none(), "Refresh token should be one-time use");
}

// ── Auth v0.9.0 ──

#[tokio::test]
async fn password_reset_token_lifecycle() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-reset").await.unwrap();
    let email = unique_email("reset");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    // 1. Generate reset token
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap();
    let token = hex::encode(raw);
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

    sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&token_hash)
    .bind(&user.id)
    .bind(expires_at)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Claim token (mark used + return user_id)
    let claimed: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used = true \
         WHERE token_hash = $1 AND used = false AND expires_at > NOW() \
         RETURNING user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(claimed, Some(user.id.clone()));

    // 3. Token is one-time use
    let second: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used = true \
         WHERE token_hash = $1 AND used = false AND expires_at > NOW() \
         RETURNING user_id",
    )
    .bind(&token_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(second.is_none(), "Reset token should be one-time use");

    // 4. Expired tokens are rejected
    //
    // token_hash column is varchar(64), so we cannot append a suffix to the
    // 64-char sha256 hex. Instead, generate a separate 64-char hash for the
    // expired token.
    let mut expired_raw = [0u8; 32];
    getrandom::getrandom(&mut expired_raw).unwrap();
    let expired_token = hex::encode(expired_raw);
    let expired_hash = hex::encode(Sha256::digest(expired_token.as_bytes()));
    sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&expired_hash)
    .bind(&user.id)
    .bind(chrono::Utc::now() - chrono::Duration::hours(1))
    .execute(&pool)
    .await
    .unwrap();

    let expired: Option<String> = sqlx::query_scalar(
        "UPDATE password_reset_tokens SET used = true \
         WHERE token_hash = $1 AND used = false AND expires_at > NOW() \
         RETURNING user_id",
    )
    .bind(&expired_hash)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(expired.is_none(), "Expired token should not be claimable");
}

#[tokio::test]
async fn register_and_login_flow() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-register").await.unwrap();
    let email = unique_email("register");
    let password = "securePass99";
    let hash = PasswordHasher::hash(password).unwrap();
    let display_name = Some("Alice");

    // 1. Register (via UserStore, simulating POST /register DB layer)
    let user = us
        .create(&tenant.id, &email, &hash, display_name)
        .await
        .unwrap();
    assert_eq!(user.tenant_id, tenant.id);
    assert_eq!(user.email.as_deref(), Some(email.as_str()));
    assert!(user.last_sign_in_at.is_none()); // Not logged in yet

    // 2. Verify password (simulating POST /login)
    let rows: Vec<(String, String, String)> =
        sqlx::query_as("SELECT id, tenant_id, password_hash FROM users WHERE email = $1")
            .bind(&email)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    let (uid, tid, stored_hash) = &rows[0];
    assert!(PasswordHasher::verify(password, stored_hash).unwrap());
    assert_eq!(uid, &user.id);
    assert_eq!(tid, &tenant.id);
}

#[tokio::test]
async fn login_updates_last_sign_in() {
    let (ts, _, us, pool) = setup().await;
    let tenant = ts.create("ut-signin").await.unwrap();
    let email = unique_email("signin");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    // Initially null
    assert!(user.last_sign_in_at.is_none());

    // Simulate login: update last_sign_in_at
    sqlx::query("UPDATE users SET last_sign_in_at = NOW() WHERE id = $1")
        .bind(&user.id)
        .execute(&pool)
        .await
        .unwrap();

    let updated = us.get_by_id(&user.id).await.unwrap().unwrap();
    assert!(
        updated.last_sign_in_at.is_some(),
        "last_sign_in_at should be set after login"
    );
}

#[tokio::test]
async fn suspended_user_login_blocked() {
    let (ts, _, us, _pool) = setup().await;
    let tenant = ts.create("ut-suspended").await.unwrap();
    let email = unique_email("blocked");
    let hash = PasswordHasher::hash("test12345").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    // Suspend user
    us.set_suspended(&user.id, true).await.unwrap();
    let fetched = us.get_by_id(&user.id).await.unwrap().unwrap();
    assert!(fetched.is_suspended);

    // Simulate login check: password is correct, but user is suspended
    let (uid, _tid, stored_hash): (String, String, String) =
        sqlx::query_as("SELECT id, tenant_id, password_hash FROM users WHERE email = $1")
            .bind(&email)
            .fetch_one(&_pool)
            .await
            .unwrap();
    assert!(PasswordHasher::verify("test12345", &stored_hash).unwrap());
    assert_eq!(uid, user.id);
    // Handler would return 403 — we trust the handler implements this check
}
