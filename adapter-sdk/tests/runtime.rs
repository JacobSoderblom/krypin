use std::sync::{Arc, Mutex};

use adapter_sdk::runtime::{spawn_command_loop, AdapterContext, AdapterLifecycle};
use anyhow::{Result, anyhow};
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        CommandSet, DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX,
        TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    model::{DeviceId, EntityDomain, EntityId},
};
use serde_json::json;
use tokio::{sync::Notify, time::{Duration, timeout}};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn adapter_context_uses_expected_topics() -> Result<()> {
    let bus: Arc<dyn Bus> = Arc::new(InMemoryBus::default());
    let ctx = AdapterContext::new(bus.clone());

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let mut device_sub = bus.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    let mut state_sub =
        bus.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    ctx.announce_device(DeviceAnnounce {
        id: device_id,
        name: "Runtime Test Device".into(),
        adapter: "runtime-test".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: Default::default(),
    })
    .await?;

    ctx.announce_entity(EntityAnnounce {
        id: entity_id,
        device_id,
        name: "Runtime Entity".into(),
        domain: EntityDomain::Switch,
        icon: None,
        key: None,
        attributes: Default::default(),
    })
    .await?;

    ctx.publish_state(StateUpdate {
        entity_id,
        value: json!(true),
        attributes: Default::default(),
        ts: chrono::Utc::now(),
        source: Some("runtime-test".into()),
    })
    .await?;

    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("device announce not received")
        .ok_or_else(|| anyhow!("device announce channel closed"))?;
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .ok_or_else(|| anyhow!("entity announce channel closed"))?;
    timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received")
        .ok_or_else(|| anyhow!("state update channel closed"))?;

    Ok(())
}

#[derive(Clone, Default)]
struct RecordingAdapter {
    observed: Arc<Mutex<Vec<CommandSet>>>,
    notify: Arc<Notify>,
}

#[async_trait::async_trait]
impl AdapterLifecycle for RecordingAdapter {
    async fn discover(&self, _ctx: &AdapterContext) -> Result<()> {
        Ok(())
    }

    async fn handle_command(
        &self,
        _ctx: &AdapterContext,
        _entity_id: EntityId,
        cmd: CommandSet,
    ) -> Result<()> {
        self.observed.lock().unwrap().push(cmd);
        self.notify.notify_waiters();
        Ok(())
    }
}

#[tokio::test]
async fn command_loop_invokes_handler() -> Result<()> {
    let bus: Arc<dyn Bus> = Arc::new(InMemoryBus::default());
    let ctx = AdapterContext::new(bus.clone());

    let entity_id = EntityId(Uuid::new_v4());
    let adapter = Arc::new(RecordingAdapter::default());

    spawn_command_loop(ctx.clone(), entity_id, adapter.clone()).await?;

    let cmd = CommandSet { action: "set".into(), value: json!({"on": true}), correlation_id: None };
    let topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    bus.publish(&topic, serde_json::to_vec(&cmd)?.into()).await?;

    timeout(Duration::from_millis(200), adapter.notify.notified())
        .await
        .expect("command handler not invoked");

    let captured = adapter.observed.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].action, "set");
    assert_eq!(captured[0].value, json!({"on": true}));

    Ok(())
}
