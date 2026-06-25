//! HTTP-level tenant isolation tests.

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
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let tenant: serde_json::Value = serde_json::from_slice(&body).unwrap();
    tenant["id"].as_str().unwrap().to_string()
}

async fn create_user(app: &axum::Router, tenant_id: &str, email: &str) -> String {
    let req = Request::builder()
        .uri("/users")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(
            json!({
                "tenant_id": tenant_id,
                "email": email,
                "password": "SecurePass123!"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    user["id"].as_str().unwrap().to_string()
}

async fn login_user(app: &axum::Router, tenant_id: &str, email: &str) -> String {
    let req = Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({"tenant_id": tenant_id, "email": email, "password": "SecurePass123!"})
                .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let login: serde_json::Value = serde_json::from_slice(&body).unwrap();
    login["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn login_lookup_lists_all_tenants_for_email() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_a = create_tenant(&app, "iso-a").await;
    let tenant_b = create_tenant(&app, "iso-b").await;
    // Same email in two tenants — allowed by ADR-016.
    let _user_a = create_user(&app, &tenant_a, "iso@example.com").await;
    let _user_b = create_user(&app, &tenant_b, "iso@example.com").await;

    let req = Request::builder()
        .uri("/login/lookup")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(json!({"email": "iso@example.com"}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tenants = result["tenants"].as_array().unwrap();
    let returned_tenant_ids: Vec<String> = tenants
        .iter()
        .map(|t| t["tenant_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        returned_tenant_ids.contains(&tenant_a),
        "tenant_a should be in lookup results"
    );
    assert!(
        returned_tenant_ids.contains(&tenant_b),
        "tenant_b should be in lookup results"
    );
}

#[tokio::test]
async fn login_credentials_are_scoped_to_tenant() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_a = create_tenant(&app, "cred-a").await;
    let tenant_b = create_tenant(&app, "cred-b").await;
    create_user(&app, &tenant_a, "cred@example.com").await;

    // Correct tenant should work.
    let req = Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({"tenant_id": tenant_a, "email": "cred@example.com", "password": "SecurePass123!"})
                .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Same email/password in a different tenant should NOT authenticate,
    // because the user only exists in tenant_a.
    let req = Request::builder()
        .uri("/login")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({"tenant_id": tenant_b, "email": "cred@example.com", "password": "SecurePass123!"})
                .to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn user_list_is_scoped_to_tenant() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_a = create_tenant(&app, "list-a").await;
    let tenant_b = create_tenant(&app, "list-b").await;
    create_user(&app, &tenant_a, "lista@example.com").await;
    create_user(&app, &tenant_b, "listb@example.com").await;

    let req = Request::builder()
        .uri(format!("/users?tenant_id={tenant_a}"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let users: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(
        users.iter().all(|u| u["tenant_id"] == tenant_a),
        "user list must not leak users from other tenants"
    );
    assert!(users.iter().any(|u| u["email"] == "lista@example.com"));
    assert!(!users.iter().any(|u| u["email"] == "listb@example.com"));
}

#[tokio::test]
async fn jwt_introspect_preserves_tenant_scoping() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_a = create_tenant(&app, "jwt-a").await;
    let tenant_b = create_tenant(&app, "jwt-b").await;
    create_user(&app, &tenant_a, "jwta@example.com").await;
    create_user(&app, &tenant_b, "jwtb@example.com").await;

    let token_a = login_user(&app, &tenant_a, "jwta@example.com").await;
    let token_b = login_user(&app, &tenant_b, "jwtb@example.com").await;

    for (token, expected_tenant) in [(&token_a, &tenant_a), (&token_b, &tenant_b)] {
        let req = Request::builder()
            .uri("/introspect")
            .method("POST")
            .header("Authorization", &common::service_token_header())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from(format!("token={token}")))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(result["active"].as_bool().unwrap());
        assert_eq!(result["tenant_id"], *expected_tenant);
    }
}
