use wavemelo::bot;
use wavemelo::config::AppConfig;
use wavemelo::error::BotError;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BotError> {
    let config = AppConfig::load()?;
    wavemelo::logging::init_logging()?;

    tracing::info!(
        lavalink_host = %config.lavalink_host,
        lavalink_port = config.lavalink_port,
        lavalink_secure = config.lavalink_secure,
        "starting wavemelo with Lavalink backend"
    );

    let mut client = bot::build_client(&config).await?;
    let shard_manager = client.shard_manager.clone();

    tokio::select! {
        result = client.start() => {
            result?;
        }
        _ = wavemelo::lifecycle::shutdown_signal() => {
            tracing::info!("shutdown signal received");
        }
    }

    shard_manager.shutdown_all().await;
    Ok(())
}
