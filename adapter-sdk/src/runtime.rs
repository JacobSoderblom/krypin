use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::{
    bus::Bus,
    bus_contract::{
        CommandSet, DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX,
        TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    model::EntityId,
};
use tokio_stream::{Stream, StreamExt};
use tracing::warn;

/// A thin wrapper around the message bus that standardizes how adapters
/// announce devices/entities, publish telemetry, and receive commands.
#[derive(Clone)]
pub struct AdapterContext {
    bus: Arc<dyn Bus>,
}

impl AdapterContext {
    pub fn new(bus: Arc<dyn Bus>) -> Self {
        Self { bus }
    }

    pub fn bus(&self) -> Arc<dyn Bus> {
        Arc::clone(&self.bus)
    }

    pub async fn announce_device(&self, announce: DeviceAnnounce) -> Result<()> {
        let bytes = Bytes::from(serde_json::to_vec(&announce)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await.context("publish device announce")
    }

    pub async fn announce_entity(&self, announce: EntityAnnounce) -> Result<()> {
        let bytes = Bytes::from(serde_json::to_vec(&announce)?);
        self.bus.publish(TOPIC_ENTITY_ANNOUNCE, bytes).await.context("publish entity announce")
    }

    pub async fn publish_state(&self, update: StateUpdate) -> Result<()> {
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (update.entity_id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await.context("publish state update")
    }

    pub async fn subscribe_commands(
        &self,
        entity_id: EntityId,
    ) -> Result<Box<dyn Stream<Item = CommandSet> + Unpin + Send>> {
        let topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
        let stream = self.bus.subscribe(&topic).await?.filter_map(move |msg| {
            let parsed = serde_json::from_slice(&msg.payload);
            if let Err(err) = &parsed {
                warn!(entity_id = ?entity_id, error = %err, "failed to decode command payload");
            }
            parsed.ok()
        });
        Ok(Box::new(stream))
    }
}

#[async_trait]
pub trait AdapterLifecycle: Send + Sync {
    async fn init(&self, _ctx: &AdapterContext) -> Result<()> {
        Ok(())
    }

    async fn discover(&self, ctx: &AdapterContext) -> Result<()>;

    async fn handle_command(
        &self,
        ctx: &AdapterContext,
        entity_id: EntityId,
        cmd: CommandSet,
    ) -> Result<()>;

    async fn telemetry_tick(&self, _ctx: &AdapterContext) -> Result<()> {
        Ok(())
    }
}

/// Helper to spawn a background command loop for an entity.
pub async fn spawn_command_loop<A>(
    ctx: AdapterContext,
    entity_id: EntityId,
    adapter: Arc<A>,
) -> Result<()>
where
    A: AdapterLifecycle + 'static,
{
    let mut cmds = ctx.subscribe_commands(entity_id).await?;
    tokio::spawn(async move {
        while let Some(cmd) = cmds.next().await {
            if let Err(err) = adapter.handle_command(&ctx, entity_id, cmd).await {
                tracing::warn!("adapter command handler failed: {err}");
            }
        }
    });
    Ok(())
}
