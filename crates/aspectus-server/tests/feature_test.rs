//! Integration tests for v0.5-v0.7 features: User, Role, OAuth2.
//! Requires DATABASE_URL and REDIS_URL env vars.

use sha2::{Digest, Sha256};
use aspectus_auth::password::PasswordHasher;
use aspectus_core::store::{TenantStore, UserStore};
use aspectus_server::db::{PgTenantStore, PgServiceAccountStore, PgUserStore};
use aspectus_server::scope_expander::ScopeExpander;

fn unique_email(prefix: &str) -> String {
    format!("{}-{}@test.com", prefix, chrono::Utc::now().timestamp_millis())
}

async fn setup() -> (PgTenantStore, PgServiceAccountStore, PgUserStore, sqlx::PgPool) {
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
    let user = us.create(&tenant.id, &email, &hash, Some("Test")).await.unwrap();
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
        .bind(&rid).bind(&user.id).bind("role_agent_dev")
        .execute(&pool).await.unwrap();

    let scopes = ScopeExpander::expand(&pool, &user.id).await;
    assert!(!scopes.is_empty(), "User should have scopes from role");
    assert!(scopes.contains("pandaria"), "Should contain pandaria scopes");
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
        .bind(&cid).bind(&user.id).bind("role_ci_deployer")
        .execute(&pool).await;

    assert!(result.is_err(), "Should reject service_account role for user");
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
    ).bind(&email).fetch_optional(&pool).await.unwrap().unwrap();
    assert!(PasswordHasher::verify("oauthpass123", &row.1).unwrap());

    // Create authorization code
    let mut raw = [0u8; 32];
    getrandom::getrandom(&mut raw).unwrap();
    let code = hex::encode(Sha256::digest(&raw));

    sqlx::query(
        "INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) \
         VALUES ($1, $2, $3, $4, $5)",
    ).bind(&code).bind(&row.0).bind("pandaria")
     .bind("https://example.com/cb")
     .bind(chrono::Utc::now() + chrono::Duration::seconds(60))
     .execute(&pool).await.unwrap();

    // Exchange code
    let token_row = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE authorization_codes SET used = true \
         WHERE code = $1 AND used = false AND expires_at > now() \
         RETURNING user_id, client_id, redirect_uri",
    ).bind(&code).fetch_optional(&pool).await.unwrap().unwrap();
    assert_eq!(token_row.0, user.id);

    // One-time use
    let second = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE authorization_codes SET used = true \
         WHERE code = $1 AND used = false AND expires_at > now() \
         RETURNING user_id, client_id, redirect_uri",
    ).bind(&code).fetch_optional(&pool).await.unwrap();
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
    let refresh = format!("rt_{}", hex::encode(&raw));
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
    ).bind(&refresh_hash).fetch_optional(&pool).await.unwrap().unwrap();
    assert_eq!(row.0, user.id);

    // One-time use
    let second = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE refresh_tokens SET revoked_at = now() \
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() \
         RETURNING user_id, client_id, token_hash",
    ).bind(&refresh_hash).fetch_optional(&pool).await.unwrap();
    assert!(second.is_none(), "Refresh token should be one-time use");
}
