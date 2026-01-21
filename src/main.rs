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
    // Install TLS crypto provider for WebSocket connections (WSS)
    // This must be done before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| eyre::eyre!("Failed to install rustls crypto provider"))?;

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

    // Initialize metrics if enabled (triggers lazy static initialization)
    if config.metrics.enabled {
        let _ = &*vixy::metrics::METRICS;
    }

    // Build the main router
    let mut app = Router::new()
        // EL HTTP proxy (with and without trailing slash)
        .route("/el", axum::routing::post(http::el_proxy_handler))
        .route("/el/", axum::routing::post(http::el_proxy_handler))
        // EL WebSocket proxy (with and without trailing slash)
        .route("/el/ws", axum::routing::get(ws::el_ws_handler))
        .route("/el/ws/", axum::routing::get(ws::el_ws_handler))
        // CL HTTP proxy (all paths under /cl/, including bare /cl and /cl/)
        .route("/cl", axum::routing::any(http::cl_proxy_handler))
        .route("/cl/", axum::routing::any(http::cl_proxy_handler))
        .route("/cl/{*path}", axum::routing::any(http::cl_proxy_handler))
        // Health endpoint for the proxy itself
        .route("/health", axum::routing::get(|| async { "OK" }))
        // Status endpoint - JSON view of all node states
        .route("/status", axum::routing::get(http::status_handler))
        .with_state(state);

    // Handle metrics based on configuration
    if config.metrics.enabled {
        if let Some(metrics_port) = config.metrics.port {
            // Spawn separate metrics server
            let metrics_addr: SocketAddr =
                format!("0.0.0.0:{metrics_port}").parse().map_err(|e| {
                    error!(error = %e, "Invalid metrics port");
                    eyre::eyre!("Invalid metrics port: {}", metrics_port)
                })?;

            tokio::spawn(async move {
                let metrics_app = Router::new().route(
                    "/metrics",
                    axum::routing::get(|| async { VixyMetrics::render() }),
                );

                info!(addr = %metrics_addr, "Starting metrics server");

                let listener = match tokio::net::TcpListener::bind(metrics_addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        error!(error = %e, "Failed to bind metrics server");
                        return;
                    }
                };

                if let Err(e) = axum::serve(listener, metrics_app).await {
                    error!(error = %e, "Metrics server error");
                }
            });
        } else {
            // Serve metrics on the main server
            app = app.route(
                "/metrics",
                axum::routing::get(|| async { VixyMetrics::render() }),
            );
        }
    }

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
