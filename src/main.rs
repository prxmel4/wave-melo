use wavemelo::bot;
use wavemelo::config::AppConfig;
use wavemelo::error::BotError;
use wavemelo::state::BotData;

// Songbird needs Symphonia's registry to register the enabled audio codecs.
use symphonia as _;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), BotError> {
    let config = AppConfig::load()?;
    wavemelo::logging::init_logging()?;

    let data = BotData::new(config)?;
    let mut client = bot::build_client(&data.config, data.clone()).await?;
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
