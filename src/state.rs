use std::collections::HashMap;
use std::sync::Arc;

use lavalink_rs::prelude::LavalinkClient;
use serenity::model::id::GuildId;
use tokio::sync::Mutex;

use crate::config::AppConfig;

/// Shared process-wide state passed to every Poise command.
#[derive(Clone)]
pub struct BotData {
    pub config: Arc<AppConfig>,
    pub lavalink: LavalinkClient,
    /// Per-guild operation locks used to serialize playback transitions.
    pub guild_ops: Arc<Mutex<HashMap<GuildId, Arc<Mutex<()>>>>>,
}

impl std::fmt::Debug for BotData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotData")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl BotData {
    /// Return the per-guild operation lock, creating it on first use.
    pub async fn guild_lock(&self, guild_id: GuildId) -> Arc<Mutex<()>> {
        self.guild_ops
            .lock()
            .await
            .entry(guild_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Drop all mutable state associated with a guild after disconnect.
    pub async fn remove_guild_state(&self, guild_id: GuildId) {
        self.guild_ops.lock().await.remove(&guild_id);
    }
}
