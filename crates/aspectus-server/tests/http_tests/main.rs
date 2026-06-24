//! HTTP integration tests for Aspectus endpoints.
//!
//! Requires DATABASE_URL and REDIS_URL environment variables.
//! Run with: `cargo test -p aspectus-server --test http_tests`

mod audit_logs_test;
mod common;
mod contract_test;
mod introspect_test;
mod management_test;
mod oauth_test;
mod service_tokens_test;
