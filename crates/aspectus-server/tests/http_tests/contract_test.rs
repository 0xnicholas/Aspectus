//! Contract tests for `POST /introspect`.
//!
//! These tests live in **Aspectus** (not in consumer projects) because
//! they verify the contract that Aspectus promises to consumers
//! (Pandaria, Tavern, Constell, Tokencamp, Heirloom). Any change
//! that breaks these tests is a **breaking change** for every consumer
//! and must be coordinated across the ecosystem.
//!
//! ## What this suite verifies
//!
//! 1. **Schema stability** — The `/introspect` response shape is
//!    pinned via [`insta`] snapshots. Renaming a field, changing a
//!    field's type, or removing a field will fail the snapshot test.
//!
//! 2. **RFC 7662 compliance** — Inactive tokens return only
//!    `{ "active": false }` (no information leakage). HTTP 200 is
//!    used for both active and inactive tokens; only authentication
//!    failures return 401.
//!
//! 3. **Header contract** — `Content-Type: application/json` on 200,
//!    `application/problem+json` on 401.
//!
//! 4. **Snapshot freshness** — When adding a NEW field to the
//!    response, update the snapshot with `cargo insta accept` and
//!    document the change in CHANGELOG.md.
//!
//! ## Running
//!
//! ```bash
//! docker compose up -d
//! DATABASE_URL=postgresql://aspectus:aspectus_dev@localhost:5433/aspectus \
//! REDIS_URL=redis://localhost:6380 \
//! cargo test -p aspectus-server --test http_tests contract_test
//! ```
//!
//! To update snapshots after an intentional contract change:
//!
//! ```bash
//! cargo insta accept    # accept all pending snapshot changes
//! cargo insta review    # interactively review each diff
//! ```
//!
//! ## Cross-reference
//!
//! - Consumer-facing documentation: `docs/consumer-integration.md` §6
//!   "错误处理矩阵" enumerates the contract from the consumer side.
//! - OpenAPI spec: `docs/openapi.yaml` `components.schemas.IntrospectResponse`
//! - ADR-001: `docs/adr/001-token-introspection-rfc7662.md`

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common;

// ───────────────────────────────────────────────────────────────────────────
// Helpers (subset of management_test.rs helpers, local to avoid coupling)
// ───────────────────────────────────────────────────────────────────────────

async fn create_tenant(app: &axum::Router, name: &str) -> String {
    let req = Request::builder()
        .uri("/tenants").method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({ "name": name }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create_tenant failed");
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: Value = serde_json::from_slice(&body).unwrap();
    tenant["id"].as_str().unwrap().to_string()
}

async fn create_service_account(app: &axum::Router, tenant_id: &str, label: &str) -> String {
    let req = Request::builder()
        .uri("/service-accounts").method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({
            "tenant_id": tenant_id,
            "label": label,
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create_service_account failed");
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let sa: Value = serde_json::from_slice(&body).unwrap();
    sa["id"].as_str().unwrap().to_string()
}

async fn create_api_key(
    app: &axum::Router,
    service_account_id: &str,
    project: &str,
    scopes: &[&str],
) -> (String, String) {
    let req = Request::builder()
        .uri("/api-keys").method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({
            "service_account_id": service_account_id,
            "project": project,
            "scopes": scopes,
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create_api_key failed");
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let key_resp: Value = serde_json::from_slice(&body).unwrap();
    let key = key_resp["key"].as_str().unwrap().to_string();
    let id = key_resp["id"].as_str().unwrap().to_string();
    (key, id)
}

async fn revoke_api_key(app: &axum::Router, key_id: &str) {
    let req = Request::builder()
        .uri(format!("/api-keys/{key_id}"))
        .method("DELETE")
        .header("Authorization", &common::service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "revoke_api_key failed");
}

async fn set_tenant_quotas(app: &axum::Router, tenant_id: &str, quotas: Value) {
    let req = Request::builder()
        .uri(format!("/tenants/{tenant_id}/quotas"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(quotas.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "set_tenant_quotas failed");
}

async fn post_introspect(app: &axum::Router, token: &str) -> (StatusCode, Value, String) {
    let req = Request::builder()
        .uri("/introspect").method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(format!("token={token}")))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = axum::body::to_bytes(resp.into_body(), 8192).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json, content_type)
}

/// Build a snapshot-friendly view of the active response.
///
/// Volatile fields (`tenant_id`, `user_id`) are replaced with placeholders so
/// snapshots remain stable across CI runs. Volatile fields are still validated
/// via separate `assert!` calls before this view is snapshotted.
fn stable_active_view(body: &Value) -> Value {
    json!({
        "active": body["active"],
        "tenant_id": "<KSUID>",
        "user_id": "<KSUID>",
        "identity_type": body["identity_type"],
        "client_id": body["client_id"],
        "scope": body["scope"],
        "token_type": body["token_type"],
        "token_format": body["token_format"],
    })
}

// ───────────────────────────────────────────────────────────────────────────
// Contract Test 1: Active API Key — the canonical contract
// ───────────────────────────────────────────────────────────────────────────
//
// This is the snapshot every consumer (Pandaria, Tavern, …) depends on.
// If this snapshot changes without coordination, every consumer breaks.

#[tokio::test]
async fn introspect_active_api_key_contract() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "contract-active-apikey").await;
    let sa_id = create_service_account(&app, &tenant_id, "contract-sa").await;
    let (api_key, _key_id) = create_api_key(
        &app,
        &sa_id,
        "pandaria",
        &["pandaria:session:create", "pandaria:session:read"],
    ).await;

    let (status, body, content_type) = post_introspect(&app, &api_key).await;

    // Header contract
    assert_eq!(status, StatusCode::OK);
    assert!(
        content_type.starts_with("application/json"),
        "expected application/json, got {content_type:?}",
    );

    // Required field shape (validated separately from snapshot)
    assert_eq!(body["active"], true);
    assert!(body["tenant_id"].is_string());
    assert!(body["user_id"].is_string(), "service_accounts must expose user_id (SA id)");
    assert_eq!(body["identity_type"], "service_account");
    assert_eq!(body["client_id"], "pandaria");
    assert_eq!(
        body["scope"], "pandaria:session:create pandaria:session:read",
        "scope must be space-separated (RFC 7662 / OAuth2)",
    );
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["token_format"], "api_key");
    assert!(body["exp"].is_null(), "API keys have no exp; must be omitted or null");
    assert!(
        body["quotas"].is_null() || body["quotas"] == json!({}),
        "no quotas set on this tenant; must be omitted or empty",
    );

    // Pin the shape — any field rename/type change will fail this assertion.
    // Volatile fields (tenant_id, user_id) are replaced with placeholders so
    // the snapshot is stable across CI runs; their presence & type are still
    // asserted above.
    insta::assert_json_snapshot!("introspect_active_api_key", stable_active_view(&body));
}

// ───────────────────────────────────────────────────────────────────────────
// Contract Test 2: Inactive (revoked) token — RFC 7662 information hiding
// ───────────────────────────────────────────────────────────────────────────
//
// Per RFC 7662 §2.2: an inactive token MUST return only `{ "active": false }`.
// Any leakage of `tenant_id`, `user_id`, or `scope` would be a privacy bug.

#[tokio::test]
async fn introspect_revoked_api_key_contract() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "contract-revoked").await;
    let sa_id = create_service_account(&app, &tenant_id, "revoked-sa").await;
    let (api_key, key_id) = create_api_key(&app, &sa_id, "pandaria", &["pandaria:session:create"]).await;

    // Verify it's active before revoking
    let (s, body, _) = post_introspect(&app, &api_key).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["active"], true);

    // Revoke and wait for cache invalidation
    revoke_api_key(&app, &key_id).await;
    // Tiny delay to ensure Redis cache TTL expires (test rig uses short TTL)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let (status, body, content_type) = post_introspect(&app, &api_key).await;

    // Header contract
    assert_eq!(status, StatusCode::OK, "RFC 7662: inactive tokens still return HTTP 200");
    assert!(content_type.starts_with("application/json"));

    // RFC 7662 §2.2: only `active: false` is allowed
    assert_eq!(body, json!({ "active": false }), "inactive response must be EXACTLY {{active: false}}");

    insta::assert_json_snapshot!("introspect_revoked_api_key", body);
}

#[tokio::test]
async fn introspect_unknown_token_contract() {
    let (app, _) = common::build_app().await.unwrap();
    let (status, body, content_type) =
        post_introspect(&app, "pk_live_00000000000000000000000000000000").await;

    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));
    assert_eq!(body, json!({ "active": false }));

    insta::assert_json_snapshot!("introspect_unknown_token", body);
}

#[tokio::test]
async fn introspect_malformed_token_contract() {
    let (app, _) = common::build_app().await.unwrap();
    let (status, body, content_type) = post_introspect(&app, "not-a-valid-token").await;

    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));
    assert_eq!(body, json!({ "active": false }));

    insta::assert_json_snapshot!("introspect_malformed_token", body);
}

// ───────────────────────────────────────────────────────────────────────────
// Contract Test 3: Active response with quotas — the `quotas` field shape
// ───────────────────────────────────────────────────────────────────────────
//
// Consumers (Pandaria, Tavern) read `quotas[<project>]` to enforce limits.
// The shape of this subtree is part of the contract.

#[tokio::test]
async fn introspect_active_with_quotas_contract() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "contract-with-quotas").await;
    let sa_id = create_service_account(&app, &tenant_id, "quotas-sa").await;
    let (api_key, _) = create_api_key(&app, &sa_id, "pandaria", &["pandaria:session:create"]).await;

    set_tenant_quotas(
        &app,
        &tenant_id,
        json!({
            "pandaria": {
                "max_concurrent_sessions": 50,
                "max_tokens_per_day": 1_000_000,
            },
            "tokencamp": {
                "monthly_tokens": 10_000_000,
            }
        }),
    ).await;

    let (status, body, content_type) = post_introspect(&app, &api_key).await;

    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));
    assert_eq!(body["active"], true);
    assert_eq!(body["quotas"]["pandaria"]["max_concurrent_sessions"], 50);
    assert_eq!(body["quotas"]["pandaria"]["max_tokens_per_day"], 1_000_000);
    assert_eq!(body["quotas"]["tokencamp"]["monthly_tokens"], 10_000_000);

    // Snapshot only the `quotas` subtree to keep the snapshot stable.
    insta::assert_json_snapshot!("introspect_quotas_subtree", body["quotas"]);
}

// ───────────────────────────────────────────────────────────────────────────
// Contract Test 4: HTTP status & header compliance
// ───────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn introspect_missing_service_token_returns_problem_details() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect").method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from("token=pk_live_anything"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("application/problem+json"),
        "401 must use RFC 7807 Problem Details (got {content_type:?})",
    );

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 401);
    assert!(json["title"].is_string());
    assert!(json["detail"].is_string());
}

#[tokio::test]
async fn introspect_wrong_service_token_returns_problem_details() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/introspect").method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", "Bearer this-is-not-the-real-service-token")
        .body(Body::from("token=pk_live_anything"))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.starts_with("application/problem+json"));
}

// ───────────────────────────────────────────────────────────────────────────
// Contract Test 5: Cross-cutting invariants
// ───────────────────────────────────────────────────────────────────────────
//
// These don't snapshot a specific shape; they enforce properties that
// must hold across ALL /introspect responses.

#[tokio::test]
async fn introspect_always_includes_active_field() {
    let (app, _) = common::build_app().await.unwrap();
    let (status, body, _) = post_introspect(&app, "anything").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
    assert!(
        body.get("active").is_some(),
        "every /introspect response must include the `active` field (RFC 7662 §2.2)",
    );
    assert!(
        body["active"].is_boolean(),
        "`active` must be a boolean, got: {}",
        body["active"],
    );
}

#[tokio::test]
async fn introspect_inactive_omits_identity_fields() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, body, _) = post_introspect(&app, "definitely-not-a-real-token").await;

    assert_eq!(body["active"], false);
    // RFC 7662 §2.2: inactive tokens must NOT leak identity
    for forbidden in ["tenant_id", "user_id", "scope", "client_id", "exp"] {
        assert!(
            body.get(forbidden).is_none() || body[forbidden].is_null(),
            "inactive response must not contain `{forbidden}` (RFC 7662 information hiding), got: {body}",
        );
    }
}
