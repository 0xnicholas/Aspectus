use axum::{extract::State, response::IntoResponse};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::AppState;

static INTROSPECT_COUNT: AtomicU64 = AtomicU64::new(0);
static INTROSPECT_ACTIVE: AtomicU64 = AtomicU64::new(0);

pub fn record_introspect(active: bool) {
    INTROSPECT_COUNT.fetch_add(1, Ordering::Relaxed);
    if active {
        INTROSPECT_ACTIVE.fetch_add(1, Ordering::Relaxed);
    }
}

pub async fn handle(State(_state): State<AppState>) -> impl IntoResponse {
    let total = INTROSPECT_COUNT.load(Ordering::Relaxed);
    let active = INTROSPECT_ACTIVE.load(Ordering::Relaxed);

    format!(
        "# HELP aspectus_introspect_total Total /introspect calls\n\
         # TYPE aspectus_introspect_total counter\n\
         aspectus_introspect_total {total}\n\
         # HELP aspectus_introspect_active Total active /introspect results\n\
         # TYPE aspectus_introspect_active counter\n\
         aspectus_introspect_active {active}\n"
    )
}
