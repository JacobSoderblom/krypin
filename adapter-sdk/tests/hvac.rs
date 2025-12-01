use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::hvac::{DeviceMeta, EntityMeta, HvacComponent, HvacDriver};
use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::hvac::{HvacCommand, HvacDescription, HvacFeatures, HvacMode, HvacState, Temperature},
    model::{DeviceId, EntityId},
};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Clone)]
struct TestDriver {
    description: HvacDescription,
    state: Arc<Mutex<HvacState>>,
    commands: Arc<Mutex<Vec<HvacCommand>>>,
}

impl TestDriver {
    fn new(description: HvacDescription, state: HvacState) -> Self {
        Self {
            description,
            state: Arc::new(Mutex::new(state)),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn commands(&self) -> Vec<HvacCommand> {
        self.commands.lock().unwrap().clone()
    }
}

#[async_trait]
impl HvacDriver for TestDriver {
    fn describe(&self) -> HvacDescription {
        self.description.clone()
    }

    async fn apply_command(&self, cmd: HvacCommand) -> Result<HvacState> {
        self.commands.lock().unwrap().push(cmd);
        Ok(self.state.lock().unwrap().clone())
    }
}

fn make_device_meta(device_id: DeviceId) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Test HVAC".into(),
        adapter: "test-adapter".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: BTreeMap::new(),
    }
}

fn make_entity_meta(entity_id: EntityId) -> EntityMeta {
    EntityMeta { id: entity_id, name: "Test HVAC".into(), icon: None, attributes: BTreeMap::new() }
}

#[tokio::test]
async fn publishes_state_update_with_expected_attributes() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = HvacDescription {
        entity_id,
        features: HvacFeatures::ONOFF | HvacFeatures::TARGET_TEMPERATURE | HvacFeatures::FAN_MODES,
        min_temp: None,
        max_temp: None,
    };

    let state = HvacState {
        mode: HvacMode::Heat,
        target_temperature: Some(Temperature(21.5)),
        ambient_temperature: Some(Temperature(19.0)),
        fan_mode: None,
    };

    let driver_impl = Arc::new(TestDriver::new(description, state.clone()));
    let component = HvacComponent::new(bus, device, entity, driver_impl);

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    component.publish_state(state.clone()).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::String("heat".into()));
    assert_eq!(update.attributes.get("target_temperature_c"), Some(&json!(21.5)));
    assert_eq!(update.attributes.get("ambient_temperature_c"), Some(&json!(19.0)));
    assert_eq!(update.source.as_deref(), Some("adapter-sdk:hvac"));

    Ok(())
}

#[tokio::test]
async fn handles_set_mode_command_and_emits_state() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = HvacDescription {
        entity_id,
        features: HvacFeatures::ONOFF | HvacFeatures::MODES,
        min_temp: None,
        max_temp: None,
    };
    let returned_state = HvacState {
        mode: HvacMode::Cool,
        target_temperature: None,
        ambient_temperature: None,
        fan_mode: None,
    };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = HvacComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;

    component.clone().spawn().await?;

    for _ in 0..2 {
        timeout(Duration::from_millis(200), device_sub.next())
            .await
            .context("device announce timed out")?
            .context("device announce channel closed")?;
    }

    let payload = Bytes::from(serde_json::to_vec(&json!({
        "action": "set_mode",
        "value": {"mode": "cool"}
    }))?);
    bus_impl.publish(&format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0), payload).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.value, serde_json::Value::String("cool".into()));
    assert_eq!(driver_impl.commands().len(), 1);

    Ok(())
}
