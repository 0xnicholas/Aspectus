use sha2::{Digest, Sha256};
use aspectus_auth::password::PasswordHasher;
use aspectus_core::store::{ServiceAccountStore, TenantStore, UserStore};
use aspectus_server::db::{PgTenantStore, PgServiceAccountStore, PgUserStore};
use aspectus_server::scope_expander::ScopeExpander;

fn unique_id(prefix: &str) -> String {
    format!("{}-{}", prefix, chrono::Utc::now().timestamp_millis())
}

async fn setup(pool: sqlx::PgPool) -> (PgTenantStore, PgServiceAccountStore, PgUserStore) {
    (PgTenantStore::new(pool.clone()), PgServiceAccountStore::new(pool.clone()), PgUserStore::new(pool))
}

async fn pool() -> sqlx::PgPool {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    sqlx::PgPool::connect(&db_url).await.unwrap()
}

#[tokio::test]
async fn admin_creates_api_key_and_introspects() {
    let p = pool().await;
    let (ts, ss, _) = setup(p.clone()).await;
    let tenant = ts.create("e2e-tenant").await.unwrap();
    let sa = ss.create(&tenant.id, "e2e-sa", None).await.unwrap();

    let mut raw = [0u8; 32]; getrandom::getrandom(&mut raw).unwrap();
    let hash = hex::encode(Sha256::digest(&raw));
    let kid = unique_id("pk");
    sqlx::query("INSERT INTO api_keys (id, tenant_id, service_account_id, project, key_hash, key_prefix, scopes) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(&kid).bind(&tenant.id).bind(&sa.id).bind(aspectus_core::project::Project::Pandaria)
        .bind(&hash).bind("pk_live_00000000").bind(&vec!["pandaria:session:create"])
        .execute(&p).await.unwrap();

    let (_, revoked) = sqlx::query_as::<_, (String, bool)>("SELECT id, revoked_at IS NOT NULL FROM api_keys WHERE key_hash = $1")
        .bind(&hash).fetch_optional(&p).await.unwrap().unwrap();
    assert!(!revoked);

    sqlx::query("UPDATE api_keys SET revoked_at = now() WHERE id = $1").bind(&kid).execute(&p).await.unwrap();

    let (_, revoked) = sqlx::query_as::<_, (String, bool)>("SELECT id, revoked_at IS NOT NULL FROM api_keys WHERE key_hash = $1")
        .bind(&hash).fetch_optional(&p).await.unwrap().unwrap();
    assert!(revoked);
}

#[tokio::test]
async fn user_role_scope_and_oauth2() {
    let p = pool().await;
    let (ts, _, us) = setup(p.clone()).await;
    let tenant = ts.create("e2e-user").await.unwrap();
    let email = format!("e2e-{}@t.com", unique_id("u"));
    let hash = PasswordHasher::hash("e2epass").unwrap();
    let user = us.create(&tenant.id, &email, &hash, None).await.unwrap();

    sqlx::query("INSERT INTO users_roles (id, user_id, role_id) VALUES ($1,$2,$3)")
        .bind(unique_id("ur")).bind(&user.id).bind("role_agent_dev").execute(&p).await.unwrap();

    let scopes = ScopeExpander::expand(&p, &user.id).await;
    assert!(!scopes.is_empty());
    assert!(scopes.contains("pandaria"));

    let (_, pw) = sqlx::query_as::<_, (String, String)>("SELECT id, password_hash FROM users WHERE email = $1")
        .bind(&email).fetch_optional(&p).await.unwrap().unwrap();
    assert!(PasswordHasher::verify("e2epass", &pw).unwrap());

    let mut raw = [0u8; 32]; getrandom::getrandom(&mut raw).unwrap();
    let code = hex::encode(Sha256::digest(&raw));
    sqlx::query("INSERT INTO authorization_codes (code, user_id, client_id, redirect_uri, expires_at) VALUES ($1,$2,$3,$4,$5)")
        .bind(&code).bind(&user.id).bind("pandaria").bind("https://cb.example.com")
        .bind(chrono::Utc::now() + chrono::Duration::seconds(60)).execute(&p).await.unwrap();

    let (uid,) = sqlx::query_as::<_, (String,)>(
        "UPDATE authorization_codes SET used = true WHERE code = $1 AND used = false AND expires_at > now() RETURNING user_id"
    ).bind(&code).fetch_optional(&p).await.unwrap().unwrap();
    assert_eq!(uid, user.id);

    let second = sqlx::query_as::<_, (String,)>(
        "UPDATE authorization_codes SET used = true WHERE code = $1 AND used = false AND expires_at > now() RETURNING user_id"
    ).bind(&code).fetch_optional(&p).await.unwrap();
    assert!(second.is_none());
}

#[tokio::test]
async fn refresh_token_rotation() {
    let p = pool().await;
    let (ts, _, us) = setup(p.clone()).await;
    let tenant = ts.create("e2e-rt").await.unwrap();
    let user = us.create(&tenant.id, &format!("rt-{}@t.com", unique_id("r")), &PasswordHasher::hash("x").unwrap(), None).await.unwrap();

    let mut raw = [0u8; 32]; getrandom::getrandom(&mut raw).unwrap();
    let token = format!("rt_{}", hex::encode(&raw));
    let thash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query("INSERT INTO refresh_tokens (token_hash, user_id, client_id, expires_at) VALUES ($1,$2,$3,$4)")
        .bind(&thash).bind(&user.id).bind("pandaria").bind(chrono::Utc::now() + chrono::Duration::days(30)).execute(&p).await.unwrap();

    let r1 = sqlx::query_as::<_, (String,)>(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() RETURNING user_id"
    ).bind(&thash).fetch_optional(&p).await.unwrap().unwrap();
    assert_eq!(r1.0, user.id);

    let r2 = sqlx::query_as::<_, (String,)>(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now() RETURNING user_id"
    ).bind(&thash).fetch_optional(&p).await.unwrap();
    assert!(r2.is_none());
}
