use serde::{Deserialize, Serialize};
use sqlx::Type;

/// Distinguishes human users from machine identities (ADR-004).
///
/// Borrowed from Logto's design, extended with `Both` for `role_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "identity_type", rename_all = "lowercase")]
pub enum IdentityType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "service_account")]
    #[sqlx(rename = "service_account")]
    ServiceAccount,
}

/// Constrains which identity types a Role can be assigned to (ADR-004, ADR-005).
///
/// Borrowed from Logto's `role_type` design, extended with `Both`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "role_type", rename_all = "lowercase")]
pub enum RoleType {
    User,
    #[sqlx(rename = "service_account")]
    ServiceAccount,
    Both,
}
