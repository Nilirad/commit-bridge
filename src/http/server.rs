//! HTTP server runtime and lifecycle.

use axum::Router;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{config::Config, error::FatalError};

/// Runs the server.
pub(crate) async fn run_server(
    app: Router,
    config: &Config,
    token: CancellationToken,
) -> Result<(), FatalError> {
    let listener = tokio::net::TcpListener::bind(config.server.address)
        .await
        .map_err(FatalError::TcpBinding)?;
    println!("Server listening on http://{}", config.server.address);
    println!(
        "Scalar UI available at http://{}/scalar",
        config.server.address
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(token))
        .await
        .map_err(FatalError::Serve)?;

    Ok(())
}

/// Creates a future that resolves when a termination signal is received.
async fn shutdown_signal(token: CancellationToken) {
    let ctrl_c = signal::ctrl_c();

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
            signal.recv().await;
        } else {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = token.cancelled() => {},
    }
    info!("Shutdown signal received, initiating graceful shutdown...");
}
