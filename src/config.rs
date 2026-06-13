/// Server configuration loaded from environment variables.
///
/// Usage:
///   PORT=8080 RUST_LOG=debug cargo run
pub struct Config {
    pub port: u16,
    pub broadcast_capacity: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),

            broadcast_capacity: std::env::var("BROADCAST_CAPACITY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1024),
        }
    }
}
