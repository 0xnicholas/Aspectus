//! HTTP-level tests for management API endpoints.
//!
//! Tests: tenants, users, api-keys, roles, service-accounts CRUD,
//! scope validation, role type constraints, audit logging.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// Create a tenant via the HTTP API, return its id.
async fn create_tenant(app: &axum::Router, name: &str) -> String {
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"name": name}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: serde_json::Value = serde_json::from_slice(&body).unwrap();
    tenant["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn tenant_crud_full_flow() {
    let (app, _) = common::build_app().await.unwrap();

    // Create
    let tenant_id = create_tenant(&app, "http-test-tenant-crud").await;
    assert!(!tenant_id.is_empty());

    // Get
    let req = Request::builder()
        .uri(format!("/tenants/{tenant_id}"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Get non-existent
    let req = Request::builder()
        .uri("/tenants/nonexistent")
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tenant_name_validation() {
    let (app, _) = common::build_app().await.unwrap();

    // Empty name → 422
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"name": ""}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Name with invalid chars → 422
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"name": "bad name!"}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn user_create_and_get() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "http-test-user").await;

    // Create user
    let email = format!(
        "http-test-{}@test.com",
        chrono::Utc::now().timestamp_millis()
    );
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": &tenant_id,
                "email": &email,
                "password": "testpass123",
                "display_name": "Test User"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let user_id = user["id"].as_str().unwrap();

    // User JSON must NOT contain password_hash
    assert!(
        user.get("password_hash").is_none(),
        "password_hash must not be exposed"
    );

    // Get user
    let req = Request::builder()
        .uri(format!("/users/{user_id}"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // List users
    let req = Request::builder()
        .uri(format!("/users?tenant_id={tenant_id}"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn user_email_validation() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "http-test-email-val").await;

    // Invalid email → 422
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "email": "not-an-email",
                "password": "testpass123"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn user_suspend_and_unsuspend() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "http-test-suspend").await;
    let email = format!("suspend-{}@test.com", chrono::Utc::now().timestamp_millis());

    // Create user
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": &tenant_id, "email": &email, "password": "testpass123"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let user_id = user["id"].as_str().unwrap();

    // Suspend
    let req = Request::builder()
        .uri(format!("/users/{user_id}/suspend"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"suspended": true}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify suspended
    let req = Request::builder()
        .uri(format!("/users/{user_id}"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(user["is_suspended"], true);

    // Unsuspend
    let req = Request::builder()
        .uri(format!("/users/{user_id}/suspend"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"suspended": false}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn api_key_create_and_revoke() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "http-test-apikey").await;

    // Create Service Account
    let req = Request::builder()
        .uri("/service-accounts")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": &tenant_id,
                "label": "test-sa",
                "description": "For testing"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let sa: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let sa_id = sa["id"].as_str().unwrap();

    // Create API Key
    let req = Request::builder()
        .uri("/api-keys")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "service_account_id": sa_id,
                "project": "pandaria",
                "scopes": ["pandaria:session:create"]
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let key_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let api_key = key_resp["key"].as_str().unwrap();
    let key_id = key_resp["id"].as_str().unwrap();
    assert!(api_key.starts_with("pk_live_"));

    // Revoke
    let req = Request::builder()
        .uri(format!("/api-keys/{key_id}"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn management_endpoints_require_auth() {
    let (app, _) = common::build_app().await.unwrap();

    let endpoints: [(&str, &str, serde_json::Value); 4] = [
        ("POST", "/tenants", json!({"name": "x"})),
        ("GET", "/tenants/t1", json!(null)),
        (
            "POST",
            "/users",
            json!({"tenant_id":"t1","email":"a@b.com","password":"test12345"}),
        ),
        (
            "POST",
            "/api-keys",
            json!({"service_account_id":"sa1","project":"pandaria","scopes":[]}),
        ),
    ];

    for (method, uri, body) in &endpoints {
        let body_str = body.to_string();
        let req = Request::builder()
            .uri(*uri)
            .method(*method)
            .header("Content-Type", "application/json")
            .body(Body::from(body_str))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{} {} should require auth",
            method,
            uri
        );
    }
}

#[tokio::test]
async fn roles_list_endpoint() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/roles")
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let roles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(roles.is_array());
}

#[tokio::test]
async fn management_rejects_consumer_service_token() {
    let (app, _) = common::build_app().await.unwrap();

    // A consumer project token (pandaria) must NOT be able to create tenants.
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(
            json!({"name": "consumer-token-test"}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
