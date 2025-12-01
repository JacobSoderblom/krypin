use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::runtime::{spawn_command_loop, AdapterContext, AdapterLifecycle};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        CommandSet, DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::switch::SwitchFeatures,
    model::{DeviceId, EntityDomain, EntityId},
};
use serde_json::json;
use tokio::time::{sleep, Duration};
use tokio_stream::StreamExt;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

#[derive(Clone)]
struct TemplateAdapter {
    device_id: DeviceId,
    entity_id: EntityId,
    state: Arc<Mutex<bool>>,
}

impl TemplateAdapter {
    fn new(device_id: DeviceId, entity_id: EntityId) -> Self {
        Self { device_id, entity_id, state: Arc::new(Mutex::new(false)) }
    }

    async fn publish_state(&self, ctx: &AdapterContext) -> Result<()> {
        let on = *self.state.lock().unwrap();
        let update = StateUpdate {
            entity_id: self.entity_id,
            value: serde_json::Value::Bool(on),
            attributes: BTreeMap::new(),
            ts: Utc::now(),
            source: Some("template-adapter".into()),
        };
        ctx.publish_state(update).await
    }
}

#[async_trait]
impl AdapterLifecycle for TemplateAdapter {
    async fn init(&self, _ctx: &AdapterContext) -> Result<()> {
        tracing::info!("adapter init");
        Ok(())
    }

    async fn discover(&self, ctx: &AdapterContext) -> Result<()> {
        tracing::info!("announcing devices");
        ctx.announce_device(DeviceAnnounce {
            id: self.device_id,
            name: "Template Switch".into(),
            adapter: "template-adapter".into(),
            manufacturer: Some("Example Co".into()),
            model: Some("SDK demo".into()),
            sw_version: Some(env!("CARGO_PKG_VERSION").into()),
            hw_version: None,
            area: None,
            metadata: BTreeMap::new(),
        })
        .await?;

        let mut attributes = BTreeMap::new();
        attributes.insert(
            "features".into(),
            (SwitchFeatures::ONOFF | SwitchFeatures::TOGGLE).bits().into(),
        );
        ctx.announce_entity(EntityAnnounce {
            id: self.entity_id,
            device_id: self.device_id,
            name: "Template Switch".into(),
            domain: EntityDomain::Switch,
            icon: Some("mdi:lightbulb".into()),
            key: Some("demo:switch:1".into()),
            attributes,
        })
        .await?;

        spawn_command_loop(ctx.clone(), self.entity_id, Arc::new(self.clone())).await?;
        self.publish_state(ctx).await
    }

    async fn handle_command(
        &self,
        ctx: &AdapterContext,
        _entity_id: EntityId,
        cmd: CommandSet,
    ) -> Result<()> {
        tracing::info!(?cmd, "received command");
        match cmd.action.as_str() {
            "toggle" => {
                let mut guard = self.state.lock().unwrap();
                *guard = !*guard;
            }
            "set" => {
                if let Some(target) = cmd.value.get("on").and_then(|v| v.as_bool()) {
                    *self.state.lock().unwrap() = target;
                }
            }
            other => tracing::warn!(action = other, "unsupported action"),
        }

        self.publish_state(ctx).await
    }

    async fn telemetry_tick(&self, ctx: &AdapterContext) -> Result<()> {
        self.publish_state(ctx).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).without_time().init();

    let bus_impl = Arc::new(InMemoryBus::default());
    let ctx = AdapterContext::new(bus_impl.clone());

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let adapter = TemplateAdapter::new(device_id, entity_id);
    adapter.init(&ctx).await?;
    adapter.discover(&ctx).await?;

    // Kick the tires by sending a command to ourselves.
    let toggle_cmd =
        CommandSet { action: "toggle".into(), value: json!(null), correlation_id: None };
    let topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    bus_impl
        .publish(&topic, serde_json::to_vec(&toggle_cmd)?.into())
        .await
        .context("publish loopback command")?;

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    if let Some(update) = state_sub.next().await {
        let parsed: StateUpdate = serde_json::from_slice(&update.payload)?;
        tracing::info!(?parsed, "observed state update");
    }

    // Keep the example alive long enough for the command loop to finish.
    sleep(Duration::from_millis(200)).await;
    Ok(())
}
