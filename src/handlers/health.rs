use axum::Json;
use serde_json::{json, Value};

/// GET /health
/// Liveness probe — returns 200 OK as long as the process is running.
/// Used by Kubernetes, Docker health checks, and load balancers.
pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
