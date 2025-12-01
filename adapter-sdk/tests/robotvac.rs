use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::robotvac::{DeviceMeta, EntityMeta, RobotVacComponent, RobotVacDriver};
use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::robotvac::{
        RobotVacCommand, RobotVacDescription, RobotVacFeatures, RobotVacState, RobotVacStatus,
    },
    model::{DeviceId, EntityId},
};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Clone)]
struct TestDriver {
    description: RobotVacDescription,
    state: Arc<Mutex<RobotVacState>>,
    commands: Arc<Mutex<Vec<RobotVacCommand>>>,
}

impl TestDriver {
    fn new(description: RobotVacDescription, state: RobotVacState) -> Self {
        Self {
            description,
            state: Arc::new(Mutex::new(state)),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn commands(&self) -> Vec<RobotVacCommand> {
        self.commands.lock().unwrap().clone()
    }
}

#[async_trait]
impl RobotVacDriver for TestDriver {
    fn describe(&self) -> RobotVacDescription {
        self.description.clone()
    }

    async fn apply_command(&self, cmd: RobotVacCommand) -> Result<RobotVacState> {
        self.commands.lock().unwrap().push(cmd);
        Ok(self.state.lock().unwrap().clone())
    }
}

fn make_device_meta(device_id: DeviceId) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Test Vacuum".into(),
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
        name: "Test Vacuum".into(),
        icon: None,
        attributes: BTreeMap::new(),
    }
}

#[tokio::test]
async fn publishes_state_update_with_expected_attributes() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = RobotVacDescription { entity_id, features: RobotVacFeatures::START };

    let state = RobotVacState {
        status: RobotVacStatus::Cleaning,
        battery_level: Some(88),
        fan_power: Some(2),
    };

    let driver_impl = Arc::new(TestDriver::new(description, state.clone()));
    let component = RobotVacComponent::new(bus, device, entity, driver_impl);

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    component.publish_state(state.clone()).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::String("cleaning".into()));
    assert_eq!(update.attributes.get("battery"), Some(&json!(88u64)));
    assert_eq!(update.attributes.get("fan_power"), Some(&json!(2u64)));
    assert_eq!(update.source.as_deref(), Some("adapter-sdk:robotvac"));

    Ok(())
}

#[tokio::test]
async fn handles_start_command_and_emits_state() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = RobotVacDescription { entity_id, features: RobotVacFeatures::START };
    let returned_state =
        RobotVacState { status: RobotVacStatus::Cleaning, battery_level: None, fan_power: None };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = RobotVacComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;

    component.clone().spawn().await?;

    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .context("device announce timed out")?
        .context("device announce channel closed")?;
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .context("entity announce timed out")?
        .context("entity announce channel closed")?;

    let payload = Bytes::from(serde_json::to_vec(&json!({ "action": "start" }))?);
    bus_impl.publish(&format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0), payload).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.value, serde_json::Value::String("cleaning".into()));
    assert_eq!(driver_impl.commands().len(), 1);

    Ok(())
}
