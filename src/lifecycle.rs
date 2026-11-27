/// Wait for SIGINT (Ctrl+C) or SIGTERM and then return so the caller can shut down.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    result = tokio::signal::ctrl_c() => {
                        if let Err(error) = result {
                            tracing::warn!(error = %error, "failed to listen for CTRL+C");
                        }
                    }
                    _ = terminate.recv() => {
                        tracing::info!("received SIGTERM");
                    }
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to install SIGTERM handler");
                if let Err(error) = tokio::signal::ctrl_c().await {
                    tracing::warn!(error = %error, "failed to listen for CTRL+C");
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %error, "failed to listen for CTRL+C");
        }
    }
}
