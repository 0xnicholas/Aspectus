//! Security-focused HTTP tests: password policy and account lockout.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

async fn create_tenant(app: &axum::Router, name: &str) -> String {
    let req = Request::builder()
        .uri("/tenants")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"name": name}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let tenant: serde_json::Value = serde_json::from_slice(&body).unwrap();
    tenant["id"].as_str().unwrap().to_string()
}

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
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    user["id"].as_str().unwrap().to_string()
}

fn login_request(email: &str, password: &str, tenant_id: &str, ip: &str) -> Request<Body> {
    Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("X-Forwarded-For", ip)
        .body(Body::from(
            json!({
                "email": email,
                "password": password,
                "tenant_id": tenant_id,
            })
            .to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn weak_password_rejected_on_user_create() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "weak-pwd").await;

    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "email": "weak@example.com",
                "password": "weak",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn weak_password_rejected_on_change_password() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "weak-change").await;
    let user_id = create_user(&app, &tenant_id, "weakchange@example.com", "OldPass123!").await;

    let req = Request::builder()
        .uri(format!("/users/{user_id}/change-password"))
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "current_password": "OldPass123!",
                "new_password": "lowercaseonly",
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn account_locks_after_consecutive_failed_logins_and_unlocks() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "lockout").await;
    let email = "lockout@example.com";
    let password = "SecurePass123!";
    let user_id = create_user(&app, &tenant_id, email, password).await;

    // Default lockout threshold is 5. Use distinct IPs to avoid rate limiting.
    for i in 1..=4 {
        let req = login_request(email, "wrong-password", &tenant_id, &format!("10.0.0.{i}"));
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "attempt {i} should return invalid credentials"
        );
    }

    // 5th failed attempt crosses the threshold and returns locked.
    let req = login_request(email, "wrong-password", &tenant_id, "10.0.0.5");
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("locked"),
        "response should indicate account lockout: {text}"
    );

    // Even the correct password is rejected while locked.
    let req = login_request(email, password, &tenant_id, "10.0.0.6");
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Admin unlock clears the lockout.
    let req = Request::builder()
        .uri(format!("/users/{user_id}/unlock"))
        .method("POST")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Login now succeeds and resets the failed-attempt counter.
    let req = login_request(email, password, &tenant_id, "10.0.0.7");
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn successful_login_resets_failed_attempt_counter() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "lockout-reset").await;
    let email = "reset@example.com";
    let password = "SecurePass123!";
    create_user(&app, &tenant_id, email, password).await;

    // Two failures, then a success.
    for i in 1..=2 {
        let req = login_request(email, "wrong-password", &tenant_id, &format!("10.1.0.{i}"));
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let req = login_request(email, password, &tenant_id, "10.1.0.3");
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Counter was reset, so it takes another 5 failures to lock.
    for i in 1..=4 {
        let req = login_request(
            email,
            "wrong-password",
            &tenant_id,
            &format!("10.1.0.{}", i + 3),
        );
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let req = login_request(email, "wrong-password", &tenant_id, "10.1.0.8");
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("locked"));
}
