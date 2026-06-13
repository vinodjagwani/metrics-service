use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::{
    handlers::{health, metrics, stats},
    state::AppState,
};

/// Builds and returns the application router with all routes registered.
///
/// Route order matters for overlapping patterns:
///   /stats/watch/{service} - static "watch" segment matched before dynamic
///   /stats/{service}/{metric} - dynamic fallback
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/metrics", post(metrics::record_metric))
        .route("/stats/watch/{service}", get(stats::watch_stats))
        .route("/stats/{service}/{metric}", get(stats::get_stats))
        .with_state(state)
}
