mod config;
mod error;
mod handlers;
mod models;
mod router;
mod state;

use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    // Self health-check mode: `metrics-service healthcheck`.
    // Used by the Docker HEALTHCHECK — the scratch image has no shell or curl,
    // so the binary probes its own /health endpoint and exits 0 (ok) or 1 (fail).
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(run_healthcheck());
    }

    // Structured logging — set RUST_LOG=debug for verbose output
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "metrics_service=debug,info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::Config::from_env();
    let state = Arc::new(state::AppState::new(config.broadcast_capacity));
    let app = router::create_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    info!("Metrics service listening on http://{addr}");
    info!("Press Ctrl+C to stop");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    info!("Server shut down gracefully");
}

/// Blocking self-probe of GET /health. Returns process exit code: 0 ok, 1 fail.
/// Uses only std so it adds no dependencies and runs without the async runtime.
fn run_healthcheck() -> i32 {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let addr = format!("127.0.0.1:{port}");

    let result = (|| -> std::io::Result<bool> {
        let mut stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        stream.write_all(
            b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )?;

        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf)?;
        Ok(buf[..n].starts_with(b"HTTP/1.1 200"))
    })();

    match result {
        Ok(true) => 0,
        _ => 1,
    }
}

/// Waits for Ctrl+C (all platforms) or SIGTERM (Unix/Linux/Mac).
/// Axum drains in-flight requests before exiting.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    // On Windows there is no SIGTERM — Ctrl+C is the only signal
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c    => info!("Received Ctrl+C, shutting down"),
        _ = terminate => info!("Received SIGTERM, shutting down"),
    }
}
