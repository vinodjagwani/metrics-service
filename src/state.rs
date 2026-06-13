use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::models::{RunningStats, StatsUpdate};

/// Shared state injected into every handler via Axum's `State` extractor.
/// Wrapped in `Arc` so it can be cloned cheaply across async tasks.
pub struct AppState {
    /// Concurrent map of metric key → running aggregation.
    /// DashMap shards its locks so thousands of writers rarely contend.
    pub stats: DashMap<String, RunningStats>,

    /// Broadcast channel: every recorded metric fans out to all SSE subscribers.
    pub publisher: broadcast::Sender<StatsUpdate>,
}

impl AppState {
    pub fn new(broadcast_capacity: usize) -> Self {
        let (publisher, _) = broadcast::channel(broadcast_capacity);
        Self {
            stats: DashMap::new(),
            publisher,
        }
    }

    /// Canonical key for a service + metric pair.
    pub fn metric_key(service_name: &str, metric_name: &str) -> String {
        format!("{service_name}:{metric_name}")
    }
}
