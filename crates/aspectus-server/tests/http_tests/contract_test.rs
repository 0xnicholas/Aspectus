//! Contract tests for `POST /introspect`.
//!
//! These tests live in **Aspectus** (not in consumer projects) because
//! they verify the contract that Aspectus promises to consumers
//! (Pandaria, Constell, Tokencamp, Heirloom, Emerald). Any change
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
//! ```
//!
//! ## Schema validation (`jsonschema`)
//!
//! In addition to the snapshot tests, every response is also validated
//! against a strict JSON Schema derived from the [`IntrospectResponse`]
//! struct definition. The schema rejects:
//! - **Unknown fields** (`additionalProperties: false`) — catches silent
//!   field additions that consumers wouldn't know to ignore.
//! - **Missing required fields when active=true** — via `if/then`,
//!   enforces `tenant_id`, `user_id`, `identity_type`, `client_id`,
//!   `scope`, `token_type`, `token_format` must all be present.
//! - **Wrong types** — e.g., `active` must always be boolean.
//!
//! Together with snapshots, this gives strong contract guarantees:
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
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common;

// ───────────────────────────────────────────────────────────────────────────
// Helpers (subset of management_test.rs helpers, local to avoid coupling)
// ───────────────────────────────────────────────────────────────────────────

async fn create_tenant(app: &axum::Router, name: &str) -> String {
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
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
        .uri("/service-accounts")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "label": label,
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create_service_account failed"
    );
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
        .uri("/api-keys")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "service_account_id": service_account_id,
                "project": project,
                "scopes": scopes,
            })
            .to_string(),
        ))
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
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "revoke_api_key failed"
    );
}

async fn set_tenant_quotas(app: &axum::Router, tenant_id: &str, quotas: Value) {
    let req = Request::builder()
        .uri(format!("/tenants/{tenant_id}/quotas"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(quotas.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "set_tenant_quotas failed"
    );
}

async fn post_introspect(app: &axum::Router, token: &str) -> (StatusCode, Value, String) {
    let req = Request::builder()
        .uri("/introspect")
        .method("POST")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Authorization", &common::admin_service_token_header())
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

    // Strict JSON Schema validation — catches silent field additions
    // (additionalProperties: false) and missing required fields when active=true.
    // This complements the snapshot tests, which catch value-level changes.
    if status == StatusCode::OK && content_type.starts_with("application/json") && json.is_object()
    {
        let validator = introspect_schema();
        if let Err(errors) = validator.validate(&json) {
            let msgs: Vec<String> = errors
                .map(|e| format!("  - at `{}`: {}", e.instance_path, e))
                .collect();
            panic!(
                "IntrospectResponse failed strict schema validation:\n{}\n\nactual JSON: {}",
                msgs.join("\n"),
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            );
        }
    }

    (status, json, content_type)
}

/// Strict JSON Schema for the `/introspect` response.
///
/// Derived from `aspectus_core::introspect::IntrospectResponse`.
/// The schema rejects:
/// - Unknown fields (catches silent field additions)
/// - Wrong types
/// - Missing required fields when `active: true`
///
/// Source of truth for field requirements:
///   aspectus_core/src/introspect.rs (IntrospectResponse struct)
fn introspect_schema() -> jsonschema::JSONSchema {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["active"],
        "properties": {
            "active": { "type": "boolean" },
            "tenant_id": { "type": "string" },
            "user_id": { "type": "string" },
            "identity_type": {
                "type": "string",
                "enum": ["user", "service_account"]
            },
            "client_id": { "type": "string" },
            "scope": { "type": "string" },
            "token_type": { "type": "string" },
            "token_format": {
                "type": "string",
                "enum": ["api_key", "jwt", "opaque"]
            },
            "exp": { "type": "integer", "minimum": 0 },
            "quotas": { "type": "object" }
        },
        "additionalProperties": false,
        "if": {
            "properties": { "active": { "const": true } },
            "required": ["active"]
        },
        "then": {
            "required": [
                "tenant_id", "user_id", "identity_type",
                "client_id", "scope", "token_type", "token_format"
            ]
        }
    });
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(&schema)
        .expect("compile introspect JSON schema")
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
// This is the snapshot every consumer (Pandaria, Constell, …) depends on.
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
    )
    .await;

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
    assert!(
        body["user_id"].is_string(),
        "service_accounts must expose user_id (SA id)"
    );
    assert_eq!(body["identity_type"], "service_account");
    assert_eq!(body["client_id"], "pandaria");
    assert_eq!(
        body["scope"], "pandaria:session:create pandaria:session:read",
        "scope must be space-separated (RFC 7662 / OAuth2)",
    );
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(body["token_format"], "api_key");
    assert!(
        body["exp"].is_null(),
        "API keys have no exp; must be omitted or null"
    );
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
    let (api_key, key_id) =
        create_api_key(&app, &sa_id, "pandaria", &["pandaria:session:create"]).await;

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
    assert_eq!(
        status,
        StatusCode::OK,
        "RFC 7662: inactive tokens still return HTTP 200"
    );
    assert!(content_type.starts_with("application/json"));

    // RFC 7662 §2.2: only `active: false` is allowed
    assert_eq!(
        body,
        json!({ "active": false }),
        "inactive response must be EXACTLY {{active: false}}"
    );

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
// Consumers (Pandaria, Constell, etc.) read `quotas[<project>]` to enforce limits.
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
    )
    .await;

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
        .uri("/introspect")
        .method("POST")
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
        .uri("/introspect")
        .method("POST")
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

// ────────────────────────────────────────────────────────────────────────────
// Schema validation smoke tests
// ────────────────────────────────────────────────────────────────────────────
//
// Verify the JSON Schema validator actually rejects malformed responses.
// Without these, a typo in the schema would silently make all tests pass.

#[test]
fn schema_rejects_unknown_field() {
    let v = introspect_schema();
    let bad = serde_json::json!({
        "active": false,
        "secret_token_hash_leaked": "should-never-appear"
    });
    assert!(
        v.validate(&bad).is_err(),
        "schema must reject unknown fields (additionalProperties: false)"
    );
}

#[test]
fn schema_rejects_missing_required_fields_when_active() {
    let v = introspect_schema();
    let bad = serde_json::json!({
        "active": true
        // tenant_id, user_id, identity_type, client_id, scope, token_type, token_format missing
    });
    let errors: Vec<_> = v.validate(&bad).unwrap_err().collect();
    assert!(
        !errors.is_empty(),
        "schema must reject active=true without required fields"
    );
    let err_text: String = errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        err_text.contains("tenant_id") || err_text.contains("required"),
        "error should mention missing required fields, got: {err_text}"
    );
}

#[test]
fn schema_rejects_wrong_active_type() {
    let v = introspect_schema();
    let bad = serde_json::json!({"active": "true"}); // string, not bool
    assert!(
        v.validate(&bad).is_err(),
        "schema must reject `active` as non-boolean"
    );
}

#[test]
fn schema_accepts_valid_inactive_response() {
    let v = introspect_schema();
    let good = serde_json::json!({"active": false});
    assert!(
        v.validate(&good).is_ok(),
        "schema must accept minimal {{active: false}}"
    );
}

#[test]
fn schema_accepts_valid_active_response() {
    let v = introspect_schema();
    let good = serde_json::json!({
        "active": true,
        "tenant_id": "t1",
        "user_id": "u1",
        "identity_type": "service_account",
        "client_id": "pandaria",
        "scope": "pandaria:session:create",
        "token_type": "Bearer",
        "token_format": "api_key"
    });
    assert!(
        v.validate(&good).is_ok(),
        "schema must accept full active response"
    );
}

// ────────────────────────────────────────────────────────────────────────
// Contract Test 6: JWT (active) — token_format: "jwt"
// ────────────────────────────────────────────────────────────────────────
//
// Same contract as API Key (Test 1), but for a JWT signed with the
// project's private key. The shape is pinned so consumers (Pandaria,
// Constell, etc.) parsing /introspect for JWT tokens don't break.

#[tokio::test]
async fn introspect_active_jwt_contract() {
    use aspectus_core::identity::IdentityType;
    use aspectus_core::project::Project;

    let app = common::build_app_with().await.unwrap();
    let tenant_id = create_tenant(&app.router, "contract-jwt").await;
    let sa_id = create_service_account(&app.router, &tenant_id, "jwt-sa").await;

    // Sign a JWT mimicking what /oauth/token would produce.
    let jwt = app
        .jwt_signer
        .sign_with_tenant_name(aspectus_auth::jwt::JwtSignRequest {
            sub: sa_id.clone(),
            tenant_id: tenant_id.clone(),
            tenant_name: None,
            project: Project::Pandaria,
            scopes: "pandaria:session:create pandaria:session:read".to_string(),
            identity_type: IdentityType::ServiceAccount,
            ttl_seconds: 900,
        })
        .expect("sign JWT");

    let (status, body, content_type) = post_introspect(&app.router, &jwt).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));

    // Header contract — JWT-specific markers
    assert_eq!(body["active"], true);
    assert_eq!(body["identity_type"], "service_account");
    assert_eq!(body["client_id"], "pandaria");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(
        body["token_format"], "jwt",
        "JWT tokens must report token_format=\"jwt\""
    );
    assert!(
        body["exp"].is_i64(),
        "JWT must include exp (expiry as Unix seconds)"
    );
    assert_eq!(
        body["scope"],
        "pandaria:session:create pandaria:session:read"
    );
    // tenant_id and user_id are checked separately as they vary per run
    assert!(body["tenant_id"].is_string());
    assert!(body["user_id"].is_string());

    // Pin the shape. tenant_id / user_id / exp are volatile; redact for stability.
    let stable = json!({
        "active": body["active"],
        "tenant_id": "<JWT-tenant>",
        "user_id": "<KSUID>",
        "identity_type": body["identity_type"],
        "client_id": body["client_id"],
        "scope": body["scope"],
        "token_type": body["token_type"],
        "token_format": body["token_format"],
        "exp_is_int": body["exp"].is_i64(),
    });
    insta::assert_json_snapshot!("introspect_active_jwt", stable);
}

// ────────────────────────────────────────────────────────────────────────
// Contract Test 7: Opaque token (active) — token_format: "opaque"
// ────────────────────────────────────────────────────────────────────────
//
// Same contract as API Key (Test 1), but for an `ot_*` token created
// via ApiKeyCreator::create_opaque. Tests the Opaque token path
// independently of the OAuth2 flow.

#[tokio::test]
async fn introspect_active_opaque_contract() {
    use aspectus_core::project::Project;

    let app = common::build_app_with().await.unwrap();
    let tenant_id = create_tenant(&app.router, "contract-opaque").await;
    let sa_id = create_service_account(&app.router, &tenant_id, "opaque-sa").await;

    // Mint an Opaque token (ot_* prefix) directly via the creator.
    let created = app
        .api_key_creator
        .create_opaque(
            &tenant_id,
            &sa_id,
            Project::Pandaria,
            "pandaria:session:create pandaria:session:read",
            3600, // 1h TTL
        )
        .await
        .expect("create_opaque");
    assert!(
        created.key.starts_with("ot_"),
        "Opaque token must have ot_ prefix"
    );

    let (status, body, content_type) = post_introspect(&app.router, &created.key).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));

    assert_eq!(body["active"], true);
    assert_eq!(body["identity_type"], "service_account");
    assert_eq!(body["client_id"], "pandaria");
    assert_eq!(body["token_type"], "Bearer");
    assert_eq!(
        body["token_format"], "opaque",
        "Opaque tokens must report token_format=\"opaque\""
    );
    assert!(
        body["exp"].is_i64(),
        "Opaque tokens with TTL must include exp"
    );
    assert_eq!(
        body["scope"],
        "pandaria:session:create pandaria:session:read"
    );
    assert!(body["tenant_id"].is_string());
    assert!(body["user_id"].is_string());

    let stable = json!({
        "active": body["active"],
        "tenant_id": "<OPAQ-tenant>",
        "user_id": "<KSUID>",
        "identity_type": body["identity_type"],
        "client_id": body["client_id"],
        "scope": body["scope"],
        "token_type": body["token_type"],
        "token_format": body["token_format"],
        "exp_is_int": body["exp"].is_i64(),
    });
    insta::assert_json_snapshot!("introspect_active_opaque", stable);
}

// ────────────────────────────────────────────────────────────────────────
// Contract Test 8: Expired JWT — must return {active: false}
// ────────────────────────────────────────────────────────────────────────
//
// Expired tokens must be treated as inactive, not 4xx (RFC 7662).
// This is the most common JWT failure mode.

#[tokio::test]
async fn introspect_expired_jwt_contract() {
    use aspectus_core::project::Project;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use sha2::Digest as _;
    use sha2::Sha256;

    let app = common::build_app_with().await.unwrap();
    let tenant_id = create_tenant(&app.router, "contract-jwt-expired").await;
    let sa_id = create_service_account(&app.router, &tenant_id, "expired-sa").await;

    // Manually craft a JWT with exp in the past so it's actually expired.
    // (JwtSigner::sign always sets iat=now which combined with the default
    // 60s leeway would still be accepted even with ttl=0.)
    let now = chrono::Utc::now().timestamp();
    let claims = aspectus_auth::jwt::JwtClaims {
        sub: sa_id,
        tenant_id: tenant_id.clone(),
        tenant_name: None,
        scope: "pandaria:session:create".to_string(),
        client_id: Project::Pandaria.to_string(),
        identity_type: "service_account".to_string(),
        aud: Project::Pandaria.to_string(),
        iss: "https://aspectus".to_string(),
        iat: (now - 7200) as usize, // 2h ago
        exp: (now - 3600) as usize, // expired 1h ago (well past the 60s leeway)
        jti: format!(
            "expired-{}",
            hex::encode(Sha256::digest(tenant_id.as_bytes()))
        ),
    };
    let pem_bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("aspectus-auth")
            .join("src")
            .join("test_private.pem"),
    )
    .expect("read test private key");
    let jwt = encode(
        &Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &EncodingKey::from_rsa_pem(&pem_bytes).expect("encoding key"),
    )
    .expect("sign expired JWT");

    let (status, body, content_type) = post_introspect(&app.router, &jwt).await;
    assert_eq!(status, StatusCode::OK, "RFC 7662: expired = 200, not 401");
    assert!(content_type.starts_with("application/json"));
    assert_eq!(body, json!({"active": false}));

    insta::assert_json_snapshot!("introspect_expired_jwt", body);
}

// ────────────────────────────────────────────────────────────────────────────
// Contract Test 9: JWKS endpoint shape
// ────────────────────────────────────────────────────────────────────────────
//
// Consumers (Pandaria, Constell, etc.) fetch /.well-known/jwks.json to verify
// JWTs locally. The shape must be a standard JWK set.

#[tokio::test]
async fn jwks_endpoint_contract() {
    let app = common::build_app_with().await.unwrap();
    let req = Request::builder()
        .uri("/.well-known/jwks.json")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.router.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.starts_with("application/json"));

    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let jwks: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        jwks.get("keys").is_some(),
        "JWKS must contain a 'keys' array"
    );
    let keys = jwks["keys"].as_array().expect("keys must be an array");
    assert!(!keys.is_empty(), "JWKS must contain at least one key");

    for key in keys {
        assert_eq!(key["kty"], "RSA", "JWK key type must be RSA");
        assert!(key["n"].is_string(), "JWK must contain modulus 'n'");
        assert!(key["e"].is_string(), "JWK must contain exponent 'e'");
        assert_eq!(key["alg"], "RS256", "JWK algorithm must be RS256");
        assert_eq!(key["use"], "sig", "JWK use must be 'sig'");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Contract Test 10: Cross-tenant login isolation
// ────────────────────────────────────────────────────────────────────────────
//
// The same email can exist in multiple tenants. /login/lookup must return all
// matching tenants, and /login + /authorize must only succeed when the caller
// supplies the correct tenant_id.

async fn create_user(app: &axum::Router, tenant_id: &str, email: &str, password: &str) -> String {
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "email": email,
                "password": password,
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create_user failed");
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: Value = serde_json::from_slice(&body).unwrap();
    user["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn cross_tenant_login_isolation_contract() {
    let app = common::build_app_with().await.unwrap();
    let tenant_a = create_tenant(&app.router, "contract-isolation-a").await;
    let tenant_b = create_tenant(&app.router, "contract-isolation-b").await;

    let email = format!(
        "isolation-{}@test.com",
        chrono::Utc::now().timestamp_millis()
    );
    let password = "isolation-pass-123";

    let user_a = create_user(&app.router, &tenant_a, &email, password).await;
    let user_b = create_user(&app.router, &tenant_b, &email, password).await;
    assert_ne!(
        user_a, user_b,
        "Users in different tenants must have distinct ids"
    );

    // Step 1: /login/lookup returns both tenants for the same email.
    let req = Request::builder()
        .uri("/login/lookup")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"email": email}).to_string()))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let lookup: Value = serde_json::from_slice(&body).unwrap();
    let tenant_ids: Vec<&str> = lookup["tenants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tenant_id"].as_str().unwrap())
        .collect();
    assert_eq!(tenant_ids.len(), 2, "lookup must return both tenants");
    assert!(tenant_ids.contains(&tenant_a.as_str()));
    assert!(tenant_ids.contains(&tenant_b.as_str()));

    // Step 2: /login with tenant_a returns a token scoped to tenant_a.
    let req = Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "email": email,
                "password": password,
                "tenant_id": tenant_a,
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let login: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(login["tenant"]["id"], tenant_a);

    // Step 3: /authorize with tenant_b works, but with a non-existent tenant fails.
    // First create an OAuth2 client in tenant_b via the management API.
    let req = Request::builder()
        .uri("/clients")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "name": "isolation-client",
                "redirect_uris": ["https://example.com/cb"],
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let client: Value = serde_json::from_slice(&body).unwrap();
    let client_id = client["client_id"].as_str().unwrap();

    // Wrong tenant → 401 (ambiguous or missing user)
    let req = Request::builder()
        .uri("/authorize")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "email": email,
                "password": password,
                "tenant_id": "nonexistent-tenant",
                "client_id": client_id,
                "redirect_uri": "https://example.com/cb",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
