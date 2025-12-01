use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        CommandSet, DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX,
        TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    model::{DeviceId, EntityDomain, EntityId},
};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

async fn spawn_mock_adapter(
    bus: Arc<dyn Bus>,
    entity_id: EntityId,
    device_id: DeviceId,
) -> Result<()> {
    // Discovery
    let device_bytes = Bytes::from(serde_json::to_vec(&DeviceAnnounce {
        id: device_id,
        name: "Mock Device".into(),
        adapter: "test-adapter".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: Default::default(),
    })?);
    bus.publish(TOPIC_DEVICE_ANNOUNCE, device_bytes).await?;

    let entity_bytes = Bytes::from(serde_json::to_vec(&EntityAnnounce {
        id: entity_id,
        device_id,
        name: "Mock Switch".into(),
        domain: EntityDomain::Switch,
        icon: None,
        key: Some("mock:1".into()),
        attributes: Default::default(),
    })?);
    bus.publish(TOPIC_ENTITY_ANNOUNCE, entity_bytes).await?;

    // Command loop
    let mut stream = bus.subscribe(&format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0)).await?;
    tokio::spawn(async move {
        while let Some(msg) = stream.next().await {
            if let Ok(cmd) = serde_json::from_slice::<CommandSet>(&msg.payload) {
                let desired = cmd.value.get("on").and_then(|v| v.as_bool()).unwrap_or(false);
                let update = StateUpdate {
                    entity_id,
                    value: serde_json::Value::Bool(desired),
                    attributes: Default::default(),
                    ts: Utc::now(),
                    source: Some("mock-adapter".into()),
                };
                let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0);
                if let Ok(bytes) = serde_json::to_vec(&update) {
                    let _ = bus.publish(&topic, Bytes::from(bytes)).await;
                }
            }
        }
    });

    Ok(())
}

#[tokio::test]
async fn bus_contract_round_trip() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let entity_id = EntityId(Uuid::new_v4());
    let device_id = DeviceId(Uuid::new_v4());

    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    spawn_mock_adapter(bus, entity_id, device_id).await?;

    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("device announce not received")
        .expect("device channel closed");
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .expect("entity channel closed");

    let command =
        CommandSet { action: "set".into(), value: json!({"on": true}), correlation_id: None };
    let topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    bus_impl.publish(&topic, Bytes::from(serde_json::to_vec(&command)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received")
        .expect("state stream closed");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.source.as_deref(), Some("mock-adapter"));

    Ok(())
}
