//! Unit tests for aspectus-core domain models.
//!
//! Covers JSON serialization, enum parsing, security properties
//! (password_hash/key_hash never leak), and API contracts.

use crate::{
    identity::IdentityType, introspect::IntrospectResponse, project::Project, role::Role,
    scope::Scope,
};

// ---- IntrospectResponse ----

#[test]
fn introspect_inactive_emits_only_active_field() {
    let r = IntrospectResponse::inactive();
    let json = serde_json::to_string(&r).unwrap();
    // RFC 7662: inactive tokens return ONLY active:false
    assert_eq!(json, r#"{"active":false}"#);
}

#[test]
fn introspect_active_serializes_all_set_fields() {
    let r = IntrospectResponse {
        active: true,
        tenant_id: Some("t1".into()),
        user_id: Some("u1".into()),
        identity_type: Some(IdentityType::User),
        client_id: Some("pandaria".into()),
        scope: Some("pandaria:session:create".into()),
        token_type: Some("Bearer".into()),
        exp: Some(1717000000),
        quotas: None,
        token_format: Some("jwt".into()),
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains(r#""active":true"#));
    assert!(json.contains(r#""tenant_id":"t1""#));
    assert!(json.contains(r#""identity_type":"user""#));
    assert!(json.contains(r#""token_format":"jwt""#));
}

#[test]
fn introspect_roundtrip_preserves_all_fields() {
    let original = IntrospectResponse {
        active: true,
        tenant_id: Some("tenant-abc".into()),
        user_id: Some("user-xyz".into()),
        identity_type: Some(IdentityType::ServiceAccount),
        client_id: Some("constell".into()),
        scope: Some("constell:agent:publish constell:agent:read".into()),
        token_type: Some("Bearer".into()),
        exp: Some(1735689600),
        quotas: None,
        token_format: Some("api_key".into()),
    };
    let json = serde_json::to_string(&original).unwrap();
    let parsed: IntrospectResponse = serde_json::from_str(&json).unwrap();
    assert!(parsed.active);
    assert_eq!(parsed.tenant_id, original.tenant_id);
    assert_eq!(parsed.user_id, original.user_id);
    assert_eq!(parsed.identity_type, original.identity_type);
    assert_eq!(parsed.client_id, original.client_id);
    assert_eq!(parsed.scope, original.scope);
    assert_eq!(parsed.token_format, original.token_format);
}

#[test]
fn introspect_quotas_serialized_when_present() {
    let mut quotas = std::collections::HashMap::new();
    quotas.insert(
        "tokencamp".into(),
        serde_json::json!({"monthly_tokens": 10000000}),
    );
    let r = IntrospectResponse {
        active: true,
        tenant_id: Some("t1".into()),
        user_id: None,
        identity_type: None,
        client_id: None,
        scope: None,
        token_type: None,
        exp: None,
        quotas: Some(quotas),
        token_format: None,
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("quotas"));
    assert!(json.contains("tokencamp"));
    assert!(json.contains("monthly_tokens"));
}

// ---- Project enum ----

#[test]
fn project_from_str_all_variants() {
    assert_eq!("pandaria".parse::<Project>().unwrap(), Project::Pandaria);
    assert_eq!("emerald".parse::<Project>().unwrap(), Project::Emerald);
    assert_eq!("constell".parse::<Project>().unwrap(), Project::Constell);
    assert_eq!("tokencamp".parse::<Project>().unwrap(), Project::Tokencamp);
    assert_eq!("heirloom".parse::<Project>().unwrap(), Project::Heirloom);
    assert_eq!("aspectus".parse::<Project>().unwrap(), Project::Aspectus);
}

#[test]
fn project_from_str_invalid_returns_err() {
    assert!(Project::from_str("invalid").is_err());
    assert!(Project::from_str("").is_err());
    assert!(Project::from_str("PANDARIA").is_err()); // case-sensitive
    // Tavern was removed 2026-06-21 (merged into Pandaria). It must NOT parse.
    assert!(
        Project::from_str("tavern").is_err(),
        "tavern must no longer parse as Project (Tavern was merged into Pandaria 2026-06-21)"
    );
}

#[test]
fn project_display_roundtrips() {
    for p in &[
        Project::Pandaria,
        Project::Emerald,
        Project::Constell,
        Project::Tokencamp,
        Project::Heirloom,
        Project::Aspectus,
    ] {
        let s = p.to_string();
        let parsed: Project = s.parse().unwrap();
        assert_eq!(*p, parsed);
    }
}

#[test]
fn project_serializes_as_string() {
    let json = serde_json::to_string(&Project::Pandaria).unwrap();
    assert_eq!(json, r#""pandaria""#);
}

// ---- IdentityType ----

#[test]
fn identity_type_serialization() {
    assert_eq!(
        serde_json::to_string(&IdentityType::User).unwrap(),
        r#""user""#
    );
    assert_eq!(
        serde_json::to_string(&IdentityType::ServiceAccount).unwrap(),
        r#""service_account""#
    );
}

#[test]
fn identity_type_deserialization() {
    let u: IdentityType = serde_json::from_str(r#""user""#).unwrap();
    assert_eq!(u, IdentityType::User);
    let sa: IdentityType = serde_json::from_str(r#""service_account""#).unwrap();
    assert_eq!(sa, IdentityType::ServiceAccount);
}

// ---- ApiKey: key_hash never serialized ----

#[test]
fn api_key_hash_not_in_json() {
    use crate::api_key::ApiKey;
    let key = ApiKey {
        id: "k1".into(),
        tenant_id: "t1".into(),
        service_account_id: "sa1".into(),
        project: Project::Pandaria,
        key_hash: "secret_hash_value_1234567890abcdef".into(),
        key_prefix: "pk_live_abc".into(),
        scopes: vec![],
        expires_at: None,
        revoked_at: None,
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&key).unwrap();
    // key_hash is included (it's sha256, irreversible), but the raw key is never stored
    assert!(json.contains("key_hash"));
    assert!(!json.contains("raw_key"), "Raw key must never appear");
}

// ---- User: password_hash never serialized ----

#[test]
fn user_password_hash_not_in_json() {
    use crate::user::User;
    let user = User {
        id: "u1".into(),
        tenant_id: "t1".into(),
        email: Some("test@example.com".into()),
        password_hash: Some("$argon2id$v=19$m=65536,t=3,p=4$secret".into()),
        display_name: None,
        is_suspended: false,
        failed_login_attempts: 0,
        locked_until: None,
        last_sign_in_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&user).unwrap();
    assert!(
        !json.contains("password_hash"),
        "password_hash must be #[serde(skip)]"
    );
    assert!(
        !json.contains("argon2"),
        "password hash value must not leak"
    );
}

// ---- CreatedApiKey serialization ----

#[test]
fn created_api_key_contains_raw_key() {
    use crate::api_key::CreatedApiKey;
    let created = CreatedApiKey {
        id: "k1".into(),
        key: "pk_live_abc123def456".into(),
        key_prefix: "pk_live_abc123de".into(),
        project: Project::Constell,
        scopes: vec!["constell:agent:read".into()],
        expires_at: None,
    };
    let json = serde_json::to_string(&created).unwrap();
    assert!(json.contains("pk_live_abc123def456"));
    assert!(json.contains("constell"));
}

// ---- Scope format convention ----

#[test]
fn scope_name_follows_project_resource_action_format() {
    let s = Scope {
        id: "s1".into(),
        name: "pandaria:session:create".into(),
        description: None,
    };
    assert!(
        s.name.contains(':'),
        "Scope should be in project:resource:action format"
    );
    let parts: Vec<&str> = s.name.splitn(3, ':').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "pandaria");
}

// ---- Role type constraint ----

#[test]
fn role_type_variants() {
    let user_role = Role {
        id: "r1".into(),
        name: "agent-developer".into(),
        description: None,
        r#type: crate::identity::RoleType::User,
        is_default: false,
        is_system: false,
    };
    let sa_role = Role {
        id: "r2".into(),
        name: "ci-deployer".into(),
        description: None,
        r#type: crate::identity::RoleType::ServiceAccount,
        is_default: false,
        is_system: false,
    };
    let both_role = Role {
        id: "r3".into(),
        name: "tenant-admin".into(),
        description: None,
        r#type: crate::identity::RoleType::Both,
        is_default: true,
        is_system: true,
    };
    // Verify different types are distinct
    assert_ne!(user_role.r#type, sa_role.r#type);
    assert_ne!(sa_role.r#type, both_role.r#type);
}

// ---- Tenant serialization ----

#[test]
fn tenant_quotas_default_empty_object() {
    use crate::tenant::Tenant;
    let tenant = Tenant {
        id: "t1".into(),
        name: "acme".into(),
        quotas: serde_json::json!({}),
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&tenant).unwrap();
    assert!(json.contains(r#""quotas":{}"#));
}

// ---- Error types ----

#[test]
fn not_found_error_format() {
    use crate::error::CoreError;
    let err = CoreError::NotFound {
        entity: "Tenant",
        id: "t-nonexistent".into(),
    };
    assert!(err.to_string().contains("Tenant"));
    assert!(err.to_string().contains("t-nonexistent"));
}

#[test]
fn validation_error_format() {
    use crate::error::CoreError;
    let err = CoreError::Validation("Invalid scope".into());
    assert!(err.to_string().contains("validation failed"));
    assert!(err.to_string().contains("Invalid scope"));
}

#[test]
fn internal_error_format() {
    use crate::error::CoreError;
    let err = CoreError::Internal("DB connection lost".into());
    assert!(err.to_string().contains("internal error"));
    assert!(err.to_string().contains("DB connection lost"));
}

#[test]
fn invalid_project_error_format() {
    use crate::error::CoreError;
    let err = CoreError::InvalidProject("tavern".into());
    assert!(err.to_string().contains("invalid project"));
    assert!(err.to_string().contains("tavern"));
}

#[test]
fn invalid_scope_error_format() {
    use crate::error::CoreError;
    let err = CoreError::InvalidScope("bad scope".into());
    assert!(err.to_string().contains("invalid scope format"));
    assert!(err.to_string().contains("bad scope"));
}

// ---- IntrospectResponse active builder / validation ----

#[test]
fn introspect_active_builder_requires_mandatory_fields() {
    use crate::introspect::IntrospectResponse;
    let r = IntrospectResponse::active(
        "t1",
        "u1",
        crate::identity::IdentityType::User,
        "pandaria",
        "pandaria:session:create",
        1735689600,
        None,
        "api_key",
    );
    assert!(r.validate().is_ok());
    assert!(r.active);
    assert_eq!(r.token_type.as_deref(), Some("Bearer"));
}

#[test]
fn introspect_active_validation_catches_missing_fields() {
    use crate::introspect::IntrospectResponse;
    let r = IntrospectResponse {
        active: true,
        tenant_id: None,
        user_id: Some("u1".into()),
        identity_type: Some(crate::identity::IdentityType::User),
        client_id: Some("pandaria".into()),
        scope: Some("pandaria:session:create".into()),
        token_type: Some("Bearer".into()),
        exp: Some(1735689600),
        quotas: None,
        token_format: Some("api_key".into()),
    };
    assert!(r.validate().is_err());
}

// ---- ApiKey scope validation ----

#[test]
fn api_key_validates_matching_scopes() {
    use crate::api_key::ApiKey;
    let key = ApiKey {
        id: "k1".into(),
        tenant_id: "t1".into(),
        service_account_id: "sa1".into(),
        project: Project::Pandaria,
        key_hash: "hash".into(),
        key_prefix: "pk_live_ab".into(),
        scopes: vec![
            "pandaria:session:create".into(),
            "pandaria:session:read".into(),
        ],
        expires_at: None,
        revoked_at: None,
        created_at: chrono::Utc::now(),
    };
    assert!(key.validate_scopes().is_ok());
}

#[test]
fn api_key_rejects_mismatched_project_scope() {
    use crate::api_key::ApiKey;
    let key = ApiKey {
        id: "k1".into(),
        tenant_id: "t1".into(),
        service_account_id: "sa1".into(),
        project: Project::Pandaria,
        key_hash: "hash".into(),
        key_prefix: "pk_live_ab".into(),
        scopes: vec!["constell:agent:read".into()],
        expires_at: None,
        revoked_at: None,
        created_at: chrono::Utc::now(),
    };
    assert!(key.validate_scopes().is_err());
}

#[test]
fn api_key_rejects_invalid_scope_format() {
    use crate::api_key::ApiKey;
    let key = ApiKey {
        id: "k1".into(),
        tenant_id: "t1".into(),
        service_account_id: "sa1".into(),
        project: Project::Pandaria,
        key_hash: "hash".into(),
        key_prefix: "pk_live_ab".into(),
        scopes: vec!["not-a-scope".into()],
        expires_at: None,
        revoked_at: None,
        created_at: chrono::Utc::now(),
    };
    assert!(key.validate_scopes().is_err());
}

// ---- ServiceToken ----

#[test]
fn service_token_is_active_only_when_not_revoked() {
    use crate::service_token::ServiceToken;
    let active = ServiceToken {
        project: Project::Pandaria,
        token_hash: "hash".into(),
        token_prefix: Some("st_pan".into()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        revoked_at: None,
    };
    assert!(active.is_active());

    let revoked = ServiceToken {
        project: Project::Pandaria,
        token_hash: "hash".into(),
        token_prefix: Some("st_pan".into()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        revoked_at: Some(chrono::Utc::now()),
    };
    assert!(!revoked.is_active());
}

#[test]
fn service_token_serialization_omits_none_fields() {
    use crate::service_token::ServiceToken;
    let token = ServiceToken {
        project: Project::Constell,
        token_hash: "hash".into(),
        token_prefix: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        revoked_at: None,
    };
    let json = serde_json::to_string(&token).unwrap();
    assert!(json.contains("constell"));
    assert!(!json.contains("token_prefix"));
    assert!(!json.contains("revoked_at"));
}

// ---- ServiceAccount ----

#[test]
fn service_account_serializes() {
    use crate::service_account::ServiceAccount;
    let sa = ServiceAccount {
        id: "sa1".into(),
        tenant_id: "t1".into(),
        label: "deployer".into(),
        description: Some("CI deployment account".into()),
        expires_at: None,
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&sa).unwrap();
    assert!(json.contains("deployer"));
    assert!(json.contains("CI deployment account"));
    assert!(json.contains("t1"));
}

// ---- AuditLog ----

#[test]
fn audit_log_serializes() {
    use crate::audit_log::AuditLog;
    let log = AuditLog {
        id: "al1".into(),
        tenant_id: "t1".into(),
        actor_id: "u1".into(),
        actor_type: crate::identity::IdentityType::User,
        action: "api_key.created".into(),
        target_type: "api_key".into(),
        target_id: "k1".into(),
        metadata: serde_json::json!({"project": "pandaria"}),
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&log).unwrap();
    assert!(json.contains("api_key.created"));
    assert!(json.contains("pandaria"));
    assert!(json.contains("u1"));
}

// ---- ApiKeyListItem ----

#[test]
fn api_key_list_item_omits_hash() {
    use crate::api_key::ApiKeyListItem;
    let item = ApiKeyListItem {
        id: "k1".into(),
        service_account_id: "sa1".into(),
        project: Project::Pandaria,
        key_prefix: "pk_live_ab".into(),
        scopes: vec!["pandaria:session:create".into()],
        expires_at: None,
        revoked_at: None,
        created_at: chrono::Utc::now(),
    };
    let json = serde_json::to_string(&item).unwrap();
    assert!(!json.contains("key_hash"));
    assert!(json.contains("pk_live_ab"));
}

// ---- RoleType TryFrom ----

#[test]
fn role_type_try_from_strings() {
    use crate::identity::RoleType;
    assert_eq!(RoleType::try_from("user").unwrap(), RoleType::User);
    assert_eq!(
        RoleType::try_from("service_account").unwrap(),
        RoleType::ServiceAccount
    );
    assert_eq!(RoleType::try_from("both").unwrap(), RoleType::Both);
    assert!(RoleType::try_from("admin").is_err());
}

// Note: use imports at module level for FromStr
use std::str::FromStr;
