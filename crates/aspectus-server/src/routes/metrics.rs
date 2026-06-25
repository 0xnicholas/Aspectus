use axum::{extract::State, response::IntoResponse};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::AppState;

static INTROSPECT_COUNT: AtomicU64 = AtomicU64::new(0);
static INTROSPECT_ACTIVE: AtomicU64 = AtomicU64::new(0);

static TOKEN_ISSUED_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOKEN_ISSUED_JWT: AtomicU64 = AtomicU64::new(0);
static TOKEN_ISSUED_OPAQUE: AtomicU64 = AtomicU64::new(0);

static TOKEN_REVOKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static TOKEN_REVOKED_SUCCESS: AtomicU64 = AtomicU64::new(0);

pub fn record_introspect(active: bool) {
    INTROSPECT_COUNT.fetch_add(1, Ordering::Relaxed);
    if active {
        INTROSPECT_ACTIVE.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_token_issued(format: &str) {
    TOKEN_ISSUED_TOTAL.fetch_add(1, Ordering::Relaxed);
    match format {
        "jwt" => TOKEN_ISSUED_JWT.fetch_add(1, Ordering::Relaxed),
        "opaque" => TOKEN_ISSUED_OPAQUE.fetch_add(1, Ordering::Relaxed),
        _ => 0,
    };
}

pub fn record_token_revoked(success: bool) {
    TOKEN_REVOKED_TOTAL.fetch_add(1, Ordering::Relaxed);
    if success {
        TOKEN_REVOKED_SUCCESS.fetch_add(1, Ordering::Relaxed);
    }
}

pub async fn handle(State(_state): State<AppState>) -> impl IntoResponse {
    let total = INTROSPECT_COUNT.load(Ordering::Relaxed);
    let active = INTROSPECT_ACTIVE.load(Ordering::Relaxed);
    let token_total = TOKEN_ISSUED_TOTAL.load(Ordering::Relaxed);
    let token_jwt = TOKEN_ISSUED_JWT.load(Ordering::Relaxed);
    let token_opaque = TOKEN_ISSUED_OPAQUE.load(Ordering::Relaxed);
    let revoked_total = TOKEN_REVOKED_TOTAL.load(Ordering::Relaxed);
    let revoked_success = TOKEN_REVOKED_SUCCESS.load(Ordering::Relaxed);

    format!(
        "# HELP aspectus_introspect_total Total /introspect calls\n\
         # TYPE aspectus_introspect_total counter\n\
         aspectus_introspect_total {total}\n\
         # HELP aspectus_introspect_active Total active /introspect results\n\
         # TYPE aspectus_introspect_active counter\n\
         aspectus_introspect_active {active}\n\
         # HELP aspectus_token_issued_total Total /token calls\n\
         # TYPE aspectus_token_issued_total counter\n\
         aspectus_token_issued_total {token_total}\n\
         # HELP aspectus_token_issued_jwt Total JWT tokens issued\n\
         # TYPE aspectus_token_issued_jwt counter\n\
         aspectus_token_issued_jwt {token_jwt}\n\
         # HELP aspectus_token_issued_opaque Total opaque tokens issued\n\
         # TYPE aspectus_token_issued_opaque counter\n\
         aspectus_token_issued_opaque {token_opaque}\n\
         # HELP aspectus_token_revoked_total Total /token/revoke calls\n\
         # TYPE aspectus_token_revoked_total counter\n\
         aspectus_token_revoked_total {revoked_total}\n\
         # HELP aspectus_token_revoked_success Total successful token revocations\n\
         # TYPE aspectus_token_revoked_success counter\n\
         aspectus_token_revoked_success {revoked_success}\n"
    )
}
