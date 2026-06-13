use serde::Serialize;

/// Internal running aggregation for a single metric.
/// Stored in the DashMap, updated on every incoming event.
#[derive(Debug, Default, Clone)]
pub struct RunningStats {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
}

impl RunningStats {
    pub fn update(&mut self, val: f64) {
        self.count += 1;
        self.sum += val;

        if self.count == 1 {
            self.min = val;
            self.max = val;
        } else {
            self.min = self.min.min(val);
            self.max = self.max.max(val);
        }
    }

    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

/// Serialisable snapshot sent to clients — via JSON response or SSE stream.
#[derive(Debug, Serialize, Clone)]
pub struct StatsUpdate {
    pub service_name: String,
    pub metric_name: String,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
}

impl StatsUpdate {
    pub fn from_stats(service_name: String, metric_name: String, s: &RunningStats) -> Self {
        Self {
            service_name,
            metric_name,
            avg: s.avg(),
            min: s.min,
            max: s.max,
            count: s.count,
        }
    }
}
