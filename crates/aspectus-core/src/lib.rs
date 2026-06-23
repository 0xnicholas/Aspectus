//! Aspectus core domain models.
//!
//! This crate defines all data types, enums, and traits used across the Aspectus
//! identity and multi-tenancy service. It has no business logic — only type
//! definitions and trait signatures.

pub mod api_key;
pub mod audit_log;
pub mod error;
pub mod error_code;
pub mod identity;
pub mod introspect;
pub mod project;
pub mod role;
pub mod scope;
pub mod service_account;
pub mod service_token;
pub mod store;
pub mod tenant;
pub mod user;
pub mod util;

pub use error_code::ErrorCode;
pub use util::generate_id;

#[cfg(test)]
mod tests;
