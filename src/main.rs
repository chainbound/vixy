//! Vixy - Ethereum EL and CL Proxy
//!
//! Entry point for the Vixy proxy server.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use clap::Parser;
use tokio::signal;
use tracing::{error, info};

use vixy::config::Config;
use vixy::metrics::VixyMetrics;
use vixy::monitor;
use vixy::proxy::{http, ws};
use vixy::state::AppState;

/// Vixy - Ethereum EL and CL Proxy
#[derive(Parser, Debug)]
#[command(name = "vixy")]
#[command(
    about = "A proxy that monitors Ethereum EL/CL nodes and routes requests to healthy nodes"
)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    /// Address to listen on
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    listen: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Parse CLI arguments
    let args = Args::parse();

    info!(config = %args.config, listen = %args.listen, "Starting Vixy");

    // Load configuration
    let config = Config::load(&args.config)?;

    info!(
        el_primary_count = config.el.primary.len(),
        el_backup_count = config.el.backup.len(),
        cl_count = config.cl.len(),
        max_el_lag = config.global.max_el_lag_blocks,
        max_cl_lag = config.global.max_cl_lag_slots,
        "Configuration loaded"
    );

    // Initialize application state
    let state = Arc::new(AppState::new(&config));

    // Spawn the health monitor background task
    let monitor_state = state.clone();
    let monitor_interval = config.global.health_check_interval_ms;
    tokio::spawn(async move {
        monitor::run_health_monitor(monitor_state, monitor_interval).await;
    });

    info!(interval_ms = monitor_interval, "Health monitor started");

    // Initialize metrics
    let metrics = Arc::new(VixyMetrics::new());
    let metrics_for_handler = metrics.clone();

    // Build the router
    let app = Router::new()
        // EL HTTP proxy
        .route("/el", axum::routing::post(http::el_proxy_handler))
        // EL WebSocket proxy
        .route("/el/ws", axum::routing::get(ws::el_ws_handler))
        // CL HTTP proxy (all paths under /cl/)
        .route("/cl/{*path}", axum::routing::any(http::cl_proxy_handler))
        // Health endpoint for the proxy itself
        .route("/health", axum::routing::get(|| async { "OK" }))
        // Metrics endpoint for Prometheus
        .route(
            "/metrics",
            axum::routing::get(move || {
                let metrics = metrics_for_handler.clone();
                async move { metrics.render() }
            }),
        )
        .with_state(state);

    // Parse listen address
    let addr: SocketAddr = args.listen.parse().map_err(|e| {
        error!(error = %e, "Invalid listen address");
        eyre::eyre!("Invalid listen address: {}", args.listen)
    })?;

    info!(addr = %addr, "Starting HTTP server");

    // Create the TCP listener
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Serve with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Vixy shut down gracefully");

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or SIGTERM)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = terminate => {
            info!("Received SIGTERM, shutting down...");
        }
    }
}
