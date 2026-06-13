use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{error::AppError, models::StatsUpdate, state::AppState};

/// GET /stats/:service/:metric
///
/// Returns the current aggregated stats for a specific metric.
/// Returns 404 if no data has been recorded for that metric yet.
pub async fn get_stats(
    Path((service, metric)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatsUpdate>, AppError> {
    let key = AppState::metric_key(&service, &metric);

    state
        .stats
        .get(&key)
        .map(|s| Json(StatsUpdate::from_stats(service, metric, &s)))
        .ok_or(AppError::NotFound)
}

/// GET /stats/watch/:service
///
/// Opens a Server-Sent Events stream. Pushes a JSON update every time
/// any metric for the given service is recorded.
/// Keep this connection open — it streams indefinitely.
pub async fn watch_stats(
    Path(service): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.publisher.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(move |msg| match msg {
        Ok(update) if update.service_name == service => {
            let data = serde_json::to_string(&update).unwrap_or_default();
            Some(Ok(Event::default().data(data)))
        }
        // Lagged or mismatched service — skip silently
        _ => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
