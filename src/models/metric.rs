use serde::Deserialize;

/// Incoming metric event from a client.
#[derive(Debug, Deserialize)]
pub struct MetricEvent {
    pub service_name: String,
    pub metric_name: String,
    pub value: f64,
}
