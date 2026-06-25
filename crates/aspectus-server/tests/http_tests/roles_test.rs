//! HTTP-level tests for role assignment and user scope expansion.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::common;

fn unique_name(base: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{base}-{nanos}")
}

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
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let user: serde_json::Value = serde_json::from_slice(&body).unwrap();
    user["id"].as_str().unwrap().to_string()
}

async fn list_roles(app: &axum::Router) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .uri("/roles")
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn find_role_by_name(roles: &[serde_json::Value], name: &str) -> String {
    roles
        .iter()
        .find(|r| r["name"].as_str() == Some(name))
        .map(|r| r["id"].as_str().unwrap().to_string())
        .unwrap_or_else(|| panic!("role {name} not found"))
}

#[tokio::test]
async fn assign_and_remove_role_updates_user_scopes() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-scope").await;
    let user_id = create_user(&app, &tenant_id, "roleuser@example.com").await;
    let roles = list_roles(&app).await;
    let role_id = find_role_by_name(&roles, "tenant-admin").await;

    // Assign role.
    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"role_id": role_id}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Scopes should now include a tenant-admin-only scope (not in default agent-developer).
    let req = Request::builder()
        .uri(format!("/users/{user_id}/scopes"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let scopes_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let scopes = scopes_resp["scopes"].as_array().expect("scopes array");
    assert!(
        scopes
            .iter()
            .any(|s| s.as_str() == Some("tokencamp:token:consume")),
        "tenant-admin role should grant tokencamp:token:consume"
    );

    // Remove role.
    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles/{role_id}"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Scopes should no longer include the removed role's scopes.
    let req = Request::builder()
        .uri(format!("/users/{user_id}/scopes"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let scopes_resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let scopes = scopes_resp["scopes"].as_array().expect("scopes array");
    assert!(
        !scopes
            .iter()
            .any(|s| s.as_str() == Some("tokencamp:token:consume")),
        "removed tenant-admin scope should disappear"
    );
}

async fn get_user_roles(app: &axum::Router, user_id: &str) -> Vec<serde_json::Value> {
    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn roles_list_includes_scopes() {
    let (app, _) = common::build_app().await.unwrap();

    let req = Request::builder()
        .uri("/roles")
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let roles: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(!roles.is_empty(), "seed roles should exist");

    let tenant_admin = roles
        .iter()
        .find(|r| r["name"] == "tenant-admin")
        .expect("tenant-admin exists");
    let scopes = tenant_admin["scopes"].as_array().expect("scopes array");
    assert!(
        scopes
            .iter()
            .any(|s| s.as_str() == Some("tokencamp:token:consume")),
        "tenant-admin should include tokencamp:token:consume"
    );
}

#[tokio::test]
async fn list_user_roles_reflects_assignments() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-list").await;
    let user_id = create_user(&app, &tenant_id, "rolelist@example.com").await;
    let roles = list_roles(&app).await;
    let role_id = find_role_by_name(&roles, "tenant-admin").await;

    let assigned = get_user_roles(&app, &user_id).await;
    assert!(
        !assigned.iter().any(|r| r["name"] == "tenant-admin"),
        "tenant-admin should not be assigned by default"
    );

    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"role_id": role_id}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let assigned = get_user_roles(&app, &user_id).await;
    assert!(
        assigned.iter().any(|r| r["name"] == "tenant-admin"),
        "assigned role should appear"
    );

    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles/{role_id}"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let assigned = get_user_roles(&app, &user_id).await;
    assert!(
        !assigned.iter().any(|r| r["name"] == "tenant-admin"),
        "removed role should disappear"
    );
}

#[tokio::test]
async fn role_assignment_rejects_consumer_service_token() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-authz").await;
    let user_id = create_user(&app, &tenant_id, "roleautz@example.com").await;

    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(json!({"role_id": "rol_xxx"}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn duplicate_role_assignment_is_idempotent() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-dup").await;
    let user_id = create_user(&app, &tenant_id, "roledup@example.com").await;
    let roles = list_roles(&app).await;
    let role_id = find_role_by_name(&roles, "agent-operator").await;

    let body_json = json!({"role_id": role_id}).to_string();
    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body_json.clone()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body_json))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "duplicate assignment should be idempotent"
    );
}

#[tokio::test]
async fn remove_nonexistent_role_assignment_returns_404() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-missing").await;
    let user_id = create_user(&app, &tenant_id, "rolemissing@example.com").await;

    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles/rol_doesnotexist"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_custom_role_and_fetch_detail() {
    let (app, _) = common::build_app().await.unwrap();

    let body = json!({
        "name": unique_name("custom-operator"),
        "description": "Custom operator role",
        "type": "user",
        "scopes": ["pandaria:session:read", "constell:agent:publish"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let role: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        role["name"]
            .as_str()
            .unwrap()
            .starts_with("custom-operator-")
    );
    assert!(!role["is_system"].as_bool().unwrap());
    let scopes = role["scopes"].as_array().unwrap();
    assert!(scopes.iter().any(|s| s == "pandaria:session:read"));
    assert!(scopes.iter().any(|s| s == "constell:agent:publish"));

    let req = Request::builder()
        .uri(format!("/roles/{}", role["id"].as_str().unwrap()))
        .method("GET")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn update_custom_role_changes_scopes() {
    let (app, _) = common::build_app().await.unwrap();

    let body = json!({
        "name": unique_name("update-test-role"),
        "type": "user",
        "scopes": ["pandaria:session:read"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let role: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = role["id"].as_str().unwrap().to_string();

    let body = json!({
        "description": "Updated description",
        "type": "service_account",
        "scopes": ["pandaria:session:create", "pandaria:session:read"]
    });
    let req = Request::builder()
        .uri(format!("/roles/{id}"))
        .method("PUT")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let role: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(role["description"], "Updated description");
    assert_eq!(role["type"], "service_account");
    let scopes = role["scopes"].as_array().unwrap();
    assert!(scopes.iter().any(|s| s == "pandaria:session:create"));
}

#[tokio::test]
async fn delete_custom_role_and_prevent_system_role_deletion() {
    let (app, _) = common::build_app().await.unwrap();

    let body = json!({
        "name": unique_name("delete-me"),
        "type": "user",
        "scopes": ["pandaria:session:read"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let role: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = role["id"].as_str().unwrap().to_string();

    let req = Request::builder()
        .uri(format!("/roles/{id}"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // System role cannot be deleted.
    let roles = list_roles(&app).await;
    let system = roles
        .iter()
        .find(|r| r["is_system"].as_bool() == Some(true))
        .expect("at least one system role exists");
    let req = Request::builder()
        .uri(format!("/roles/{}", system["id"].as_str().unwrap()))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cannot_delete_role_assigned_to_user() {
    let (app, _) = common::build_app().await.unwrap();
    let tenant_id = create_tenant(&app, "role-in-use").await;
    let user_id = create_user(&app, &tenant_id, "roleinuse@example.com").await;

    let body = json!({
        "name": unique_name("in-use-role"),
        "type": "user",
        "scopes": ["pandaria:session:read"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), 65536).await.unwrap();
    let role: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = role["id"].as_str().unwrap().to_string();

    // Assign to user.
    let req = Request::builder()
        .uri(format!("/users/{user_id}/roles"))
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(json!({"role_id": id}).to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let req = Request::builder()
        .uri(format!("/roles/{id}"))
        .method("DELETE")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_role_rejects_invalid_scope() {
    let (app, _) = common::build_app().await.unwrap();

    let body = json!({
        "name": unique_name("bad-scope-role"),
        "type": "user",
        "scopes": ["invalid-scope"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::admin_service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn role_crud_rejects_consumer_service_token() {
    let (app, _) = common::build_app().await.unwrap();

    let body = json!({
        "name": "consumer-role",
        "type": "user",
        "scopes": ["pandaria:session:read"]
    });
    let req = Request::builder()
        .uri("/roles")
        .method("POST")
        .header("Content-Type", "application/json")
        .header("Authorization", &common::service_token_header())
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
