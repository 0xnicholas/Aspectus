//! HTTP-level tests for OAuth2 /authorize, /token, /clients endpoints.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

/// Helper: create a tenant and user for OAuth tests.
async fn setup_user(app: &axum::Router) -> (String, String, String) {
    // Create tenant
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({"name": "oauth-test-tenant"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tenant_id = tenant["id"].as_str().unwrap().to_string();

    // Create user
    let email = format!("oauth-test-{}@test.com", chrono::Utc::now().timestamp_millis());
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({
            "tenant_id": &tenant_id,
            "email": &email,
            "password": "oauth-password-123",
            "display_name": "OAuth Test User"
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let _user_id = user["id"].as_str().unwrap().to_string();

    // Register OAuth2 client
    let req = Request::builder()
        .uri("/clients")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({
            "name": "test-client",
            "redirect_uris": ["https://example.com/cb"]
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let client: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let client_id = client["client_id"].as_str().unwrap().to_string();

    (tenant_id, email, client_id)
}

#[tokio::test]
async fn list_clients_requires_auth() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/clients")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn authorize_wrong_password_returns_401() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, email, client_id) = setup_user(&app).await;

    let req = Request::builder()
        .uri("/authorize")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "email": email,
            "password": "wrong-password",
            "client_id": client_id,
            "redirect_uri": "https://example.com/cb"
        }).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn authorize_invalid_client_returns_422() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, email, _) = setup_user(&app).await;

    let req = Request::builder()
        .uri("/authorize")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "email": email,
            "password": "oauth-password-123",
            "client_id": "nonexistent-client",
            "redirect_uri": "https://example.com/cb"
        }).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn authorize_invalid_redirect_uri_returns_422() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, email, client_id) = setup_user(&app).await;

    let req = Request::builder()
        .uri("/authorize")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "email": email,
            "password": "oauth-password-123",
            "client_id": client_id,
            "redirect_uri": "https://evil.com/steal"
        }).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn full_oauth2_authorization_code_flow() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, email, client_id) = setup_user(&app).await;

    // Step 1: Authorize — get authorization code
    let req = Request::builder()
        .uri("/authorize")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "email": &email,
            "password": "oauth-password-123",
            "client_id": &client_id,
            "redirect_uri": "https://example.com/cb"
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let auth_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let code = auth_resp["code"].as_str().unwrap();

    // Step 2: Exchange code for tokens
    let req = Request::builder()
        .uri("/oauth/token")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "grant_type": "authorization_code",
            "code": code,
            "client_id": client_id
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let token_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(token_resp["access_token"].as_str().is_some());
    assert_eq!(token_resp["token_format"], "jwt");
    assert_eq!(token_resp["token_type"], "Bearer");
    assert!(token_resp["refresh_token"].as_str().is_some());
}

#[tokio::test]
async fn authorization_code_is_one_time_use() {
    let (app, _) = common::build_app().await.unwrap();
    let (_, email, client_id) = setup_user(&app).await;

    // Get code
    let req = Request::builder()
        .uri("/authorize").method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({
            "email": &email, "password": "oauth-password-123",
            "client_id": &client_id, "redirect_uri": "https://example.com/cb"
        }).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let code = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["code"].as_str().unwrap().to_string();

    // First exchange — succeeds
    let req = Request::builder().uri("/oauth/token").method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"grant_type":"authorization_code","code":&code,"client_id":&client_id}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Second exchange — must fail
    let req = Request::builder().uri("/oauth/token").method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"grant_type":"authorization_code","code":&code,"client_id":&client_id}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_unsupported_grant_type_returns_422() {
    let (app, _) = common::build_app().await.unwrap();
    let req = Request::builder()
        .uri("/oauth/token").method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"grant_type":"password"}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
