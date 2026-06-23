//! Unit tests for aspectus-server (no DB/Redis required).

use crate::error::{ProblemDetails, ValidationError};
use crate::util::generate_id;

// ---- ProblemDetails RFC 7807 format ----

#[test]
fn problem_details_unauthorized_format() {
    let pd = ProblemDetails::unauthorized("Invalid token", "/introspect");
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 401);
    assert_eq!(json["title"], "Unauthorized");
    assert_eq!(json["detail"], "Invalid token");
    assert_eq!(json["code"], "invalid_credentials");
    assert!(json["type"].as_str().unwrap().contains("invalid_credentials"));
    assert!(json["instance"].as_str().unwrap().contains("/introspect"));
}

#[test]
fn problem_details_forbidden_format() {
    let pd = ProblemDetails::forbidden("Access denied");
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 403);
    assert_eq!(json["title"], "Forbidden");
}

#[test]
fn problem_details_not_found_format() {
    let pd = ProblemDetails::not_found("Tenant t1 not found");
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 404);
    assert_eq!(json["title"], "Not Found");
}

#[test]
fn problem_details_validation_failed_format() {
    let errors = vec![
        crate::error::ValidationError { field: "email".into(), message: "Invalid format".into() },
        crate::error::ValidationError { field: "name".into(), message: "Too long".into() },
    ];
    let pd = ProblemDetails::validation_failed("Validation failed", errors);
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 422);
    assert_eq!(json["title"], "Validation Failed");
    let errs = json["errors"].as_array().unwrap();
    assert_eq!(errs.len(), 2);
    assert_eq!(errs[0]["field"], "email");
}

#[test]
fn problem_details_internal_error_format() {
    let pd = ProblemDetails::internal_error("DB connection lost");
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 500);
    assert_eq!(json["title"], "Internal Server Error");
}

#[test]
fn problem_details_no_instance_when_not_provided() {
    let pd = ProblemDetails::not_found("Something");
    let json = serde_json::to_value(&pd).unwrap();
    assert!(json.get("instance").is_none());
}

#[test]
fn problem_details_no_errors_when_not_provided() {
    let pd = ProblemDetails::unauthorized("Bad", "/test");
    let json = serde_json::to_value(&pd).unwrap();
    assert!(json.get("errors").is_none());
}

// ---- generate_id ----

#[test]
fn generate_id_returns_27_chars() {
    let id = generate_id();
    assert_eq!(id.len(), 27, "KSUID base62 is always 27 characters");
}

#[test]
fn generate_id_is_base62_string() {
    let id = generate_id();
    // KSUID uses base62: [0-9A-Za-z]
    assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[test]
fn generate_id_is_unique() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..1000 {
        ids.insert(generate_id());
    }
    assert_eq!(ids.len(), 1000, "1000 generated IDs must all be unique");
}

// ---- ProblemDetails with ErrorCode ----

#[test]
fn problem_details_with_code_includes_code_field() {
    let pd = ProblemDetails::with_code(aspectus_core::ErrorCode::UserNotFound, "User abc not found");
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["code"], "user_not_found");
    assert_eq!(json["status"], 404);
    assert_eq!(json["title"], "Not Found");
    assert!(json["type"].as_str().unwrap().contains("user_not_found"));
}

#[test]
fn problem_details_with_code_instance() {
    let pd = ProblemDetails::with_code_instance(
        aspectus_core::ErrorCode::InvalidServiceToken,
        "Missing header",
        "/api-keys",
    );
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["code"], "invalid_service_token");
    assert_eq!(json["instance"], "/api-keys");
    assert_eq!(json["status"], 401);
}

#[test]
fn problem_details_with_code_errors() {
    let pd = ProblemDetails::with_code_errors(
        aspectus_core::ErrorCode::InvalidEmailFormat,
        "Bad email",
        vec![ValidationError { field: "email".into(), message: "no @".into() }],
    );
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["code"], "invalid_email_format");
    assert_eq!(json["status"], 422);
    let errs = json["errors"].as_array().unwrap();
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0]["field"], "email");
}

// ---- CoreError to ProblemDetails conversion ----

#[test]
fn core_error_not_found_to_problem_details() {
    use aspectus_core::error::CoreError;
    let err = CoreError::NotFound { entity: "User", id: "u1".into() };
    let pd = ProblemDetails::from(err);
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 404);
    assert!(json["detail"].as_str().unwrap().contains("User"));
    assert!(json["detail"].as_str().unwrap().contains("u1"));
}

#[test]
fn core_error_validation_to_problem_details() {
    use aspectus_core::error::CoreError;
    let err = CoreError::Validation("Invalid input".into());
    let pd = ProblemDetails::from(err);
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 422);
}

#[test]
fn core_error_internal_to_problem_details() {
    use aspectus_core::error::CoreError;
    let err = CoreError::Internal("Something broke".into());
    let pd = ProblemDetails::from(err);
    let json = serde_json::to_value(&pd).unwrap();
    assert_eq!(json["status"], 500);
}
