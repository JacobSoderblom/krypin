use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::{
    meta::{DeviceMeta, EntityMeta},
    switch::{SwitchComponent, SwitchDriver},
};
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::switch::{SwitchCommand, SwitchDescription, SwitchFeatures, SwitchState},
    model::{DeviceId, EntityId},
};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Clone)]
struct TestDriver {
    description: SwitchDescription,
    state: Arc<Mutex<SwitchState>>,
    commands: Arc<Mutex<Vec<SwitchCommand>>>,
}

impl TestDriver {
    fn new(description: SwitchDescription, state: SwitchState) -> Self {
        Self {
            description,
            state: Arc::new(Mutex::new(state)),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn commands(&self) -> Vec<SwitchCommand> {
        self.commands.lock().unwrap().clone()
    }
}

#[async_trait]
impl SwitchDriver for TestDriver {
    fn describe(&self) -> SwitchDescription {
        self.description.clone()
    }

    async fn apply_command(&self, cmd: SwitchCommand) -> Result<SwitchState> {
        self.commands.lock().unwrap().push(cmd);
        Ok(self.state.lock().unwrap().clone())
    }
}

fn make_device_meta(device_id: DeviceId) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Test Switch".into(),
        adapter: "test-adapter".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: BTreeMap::new(),
        zigbee: None,
    }
}

fn make_entity_meta(entity_id: EntityId) -> EntityMeta {
    EntityMeta {
        id: entity_id,
        name: "Test Switch".into(),
        icon: None,
        attributes: BTreeMap::new(),
    }
}

#[tokio::test]
async fn publishes_state_update_for_switch() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = SwitchDescription {
        entity_id,
        features: SwitchFeatures::ONOFF | SwitchFeatures::POWER_METER,
    };
    let state = SwitchState { on: true, power_w: Some(12.5) };
    let driver_impl = Arc::new(TestDriver::new(description, state.clone()));
    let component = SwitchComponent::new(bus, device, entity, driver_impl);

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    component.publish_state(state).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.attributes.get("power_w").and_then(|v| v.as_f64()), Some(12.5));
    assert_eq!(update.source.as_deref(), Some("adapter-sdk:switch"));

    Ok(())
}

#[tokio::test]
async fn handles_set_command_and_emits_state() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = SwitchDescription { entity_id, features: SwitchFeatures::ONOFF };
    let returned_state = SwitchState { on: true, power_w: None };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = SwitchComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;

    component.clone().spawn().await?;

    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("device announce not received")
        .expect("announce stream closed unexpectedly");
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .expect("entity stream closed unexpectedly");

    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    let command_payload = json!({ "action": "set", "value": { "on": true } });
    bus_impl.publish(&command_topic, Bytes::from(serde_json::to_vec(&command_payload)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(driver_impl.commands(), vec![SwitchCommand::Set { on: true }]);

    Ok(())
}

#[tokio::test]
async fn handles_toggle_command() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description =
        SwitchDescription { entity_id, features: SwitchFeatures::ONOFF | SwitchFeatures::TOGGLE };
    let toggled_state = SwitchState { on: false, power_w: None };
    let driver_impl = Arc::new(TestDriver::new(description, toggled_state));
    let component = SwitchComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;

    component.clone().spawn().await?;

    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("device announce not received")
        .expect("announce stream closed unexpectedly");
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .expect("entity stream closed unexpectedly");

    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    let command_payload = json!({ "action": "toggle" });
    bus_impl.publish(&command_topic, Bytes::from(serde_json::to_vec(&command_payload)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(false));
    assert_eq!(driver_impl.commands(), vec![SwitchCommand::Toggle]);

    Ok(())
}
