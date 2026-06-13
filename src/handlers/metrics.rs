use axum::{extract::State, Json};
use std::sync::Arc;
use tracing::info;

use crate::{
    error::AppError,
    models::{MetricEvent, StatsUpdate},
    state::AppState,
};

/// POST /metrics
///
/// Records a single metric event and returns the updated aggregated stats.
/// Broadcasts the update to all active SSE subscribers.
///
/// Body: `{ "service_name": "payments", "metric_name": "latency_ms", "value": 42.5 }`
pub async fn record_metric(
    State(state): State<Arc<AppState>>,
    Json(event): Json<MetricEvent>,
) -> Result<Json<StatsUpdate>, AppError> {
    let key = AppState::metric_key(&event.service_name, &event.metric_name);

    state
        .stats
        .entry(key.clone())
        .or_default()
        .update(event.value);

    let update = state
        .stats
        .get(&key)
        .map(|s| StatsUpdate::from_stats(event.service_name.clone(), event.metric_name.clone(), &s))
        .ok_or_else(|| AppError::Internal("stats missing after write".into()))?;

    // Ignore send errors — they just mean no subscribers are connected yet
    let _ = state.publisher.send(update.clone());

    info!(
        service = %event.service_name,
        metric  = %event.metric_name,
        value   = %event.value,
        count   = %update.count,
        avg     = %update.avg,
        "metric recorded"
    );

    Ok(Json(update))
}
