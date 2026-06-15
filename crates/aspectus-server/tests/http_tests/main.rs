//! HTTP integration tests for Aspectus endpoints.
//!
//! Requires DATABASE_URL and REDIS_URL environment variables.
//! Run with: `cargo test -p aspectus-server --test http_tests`

mod common;
mod introspect_test;
mod oauth_test;
mod management_test;
