use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::{
    light::{LightComponent, LightDriver},
    meta::{DeviceMeta, EntityMeta},
};
use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use hub_core::{
    bus::{Bus, InMemoryBus, Message},
    bus_contract::{
        StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::light::{
        Brightness, LightColor, LightCommand, LightDescription, LightFeatures, LightState, Mireds,
        Power, Rgb,
    },
    model::{DeviceId, EntityId},
};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Clone)]
struct TestDriver {
    description: LightDescription,
    state: Arc<Mutex<LightState>>,
    commands: Arc<Mutex<Vec<LightCommand>>>,
}

impl TestDriver {
    fn new(description: LightDescription, state: LightState) -> Self {
        Self {
            description,
            state: Arc::new(Mutex::new(state)),
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn commands(&self) -> Vec<LightCommand> {
        self.commands.lock().unwrap().clone()
    }
}

#[async_trait]
impl LightDriver for TestDriver {
    fn describe(&self) -> LightDescription {
        self.description.clone()
    }

    async fn apply_command(&self, cmd: LightCommand) -> Result<LightState> {
        self.commands.lock().unwrap().push(cmd);
        Ok(self.state.lock().unwrap().clone())
    }
}

fn make_device_meta(device_id: DeviceId) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Test Light".into(),
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
    EntityMeta { id: entity_id, name: "Test Light".into(), icon: None, attributes: BTreeMap::new() }
}

async fn wait_for_announcements(
    device_sub: &mut (impl tokio_stream::Stream<Item = Message> + Unpin),
    entity_sub: &mut (impl tokio_stream::Stream<Item = Message> + Unpin),
) -> Result<()> {
    timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("device announce not received")
        .expect("announce stream closed unexpectedly");
    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .expect("announce stream closed unexpectedly");
    Ok(())
}

#[tokio::test]
async fn publishes_state_update_with_expected_attributes() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF | LightFeatures::DIMMABLE | LightFeatures::RGB,
        min_mireds: Some(Mireds(153)),
        max_mireds: Some(Mireds(500)),
    };

    let brightness = Brightness(42);
    let rgb_values = (12, 34, 56);
    let state = LightState {
        power: Power::On,
        brightness: Some(brightness),
        color: Some(LightColor::Rgb {
            rgb: Rgb { r: rgb_values.0, g: rgb_values.1, b: rgb_values.2 },
        }),
    };

    let driver_impl = Arc::new(TestDriver::new(description, state.clone()));
    let component = LightComponent::new(bus, device, entity, driver_impl);

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    component.publish_state(state.clone()).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(
        update.attributes.get("brightness").and_then(|v| v.as_u64()),
        Some(brightness.0 as u64)
    );
    assert_eq!(
        update.attributes.get("rgb"),
        Some(&json!([rgb_values.0, rgb_values.1, rgb_values.2]))
    );
    assert_eq!(update.source.as_deref(), Some("adapter-sdk:light"));

    Ok(())
}

#[tokio::test]
async fn handles_set_power_command_and_emits_state() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF,
        min_mireds: None,
        max_mireds: None,
    };
    let returned_state = LightState { power: Power::On, brightness: None, color: None };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = LightComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    component.clone().spawn().await?;
    wait_for_announcements(&mut device_sub, &mut entity_sub).await?;

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
    assert_eq!(driver_impl.commands(), vec![LightCommand::SetPower { on: true }]);

    Ok(())
}

#[tokio::test]
async fn handles_set_brightness_command_with_transition() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF | LightFeatures::DIMMABLE,
        min_mireds: None,
        max_mireds: None,
    };
    let returned_state =
        LightState { power: Power::On, brightness: Some(Brightness(80)), color: None };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = LightComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    component.clone().spawn().await?;
    wait_for_announcements(&mut device_sub, &mut entity_sub).await?;

    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    let command_payload =
        json!({ "action": "set", "value": { "brightness": 80, "transition_ms": 500 } });
    bus_impl.publish(&command_topic, Bytes::from(serde_json::to_vec(&command_payload)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.attributes.get("brightness").and_then(|v| v.as_u64()), Some(80));
    assert_eq!(
        driver_impl.commands(),
        vec![LightCommand::SetBrightness { level: Brightness(80), transition_ms: Some(500) }]
    );

    Ok(())
}

#[tokio::test]
async fn handles_set_color_temp_command_with_transition() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF | LightFeatures::COLOR_TEMP,
        min_mireds: Some(Mireds(200)),
        max_mireds: Some(Mireds(400)),
    };
    let returned_state = LightState {
        power: Power::On,
        brightness: None,
        color: Some(LightColor::Temperature { mireds: Mireds(320) }),
    };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = LightComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    component.clone().spawn().await?;
    wait_for_announcements(&mut device_sub, &mut entity_sub).await?;

    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    let command_payload =
        json!({ "action": "set", "value": { "mireds": 320, "transition_ms": 250 } });
    bus_impl.publish(&command_topic, Bytes::from(serde_json::to_vec(&command_payload)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.attributes.get("mireds").and_then(|v| v.as_u64()), Some(320));
    assert_eq!(
        driver_impl.commands(),
        vec![LightCommand::SetColorTemp { mireds: Mireds(320), transition_ms: Some(250) }]
    );

    Ok(())
}

#[tokio::test]
async fn handles_set_rgb_command_with_transition() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF | LightFeatures::RGB,
        min_mireds: None,
        max_mireds: None,
    };
    let returned_state = LightState {
        power: Power::On,
        brightness: None,
        color: Some(LightColor::Rgb { rgb: Rgb { r: 1, g: 2, b: 3 } }),
    };
    let driver_impl = Arc::new(TestDriver::new(description, returned_state));
    let component = LightComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;
    component.clone().spawn().await?;
    wait_for_announcements(&mut device_sub, &mut entity_sub).await?;

    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    let command_payload =
        json!({ "action": "set", "value": { "rgb": [1, 2, 3], "transition_ms": 1000 } });
    bus_impl.publish(&command_topic, Bytes::from(serde_json::to_vec(&command_payload)?)).await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("state update not received in time")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.attributes.get("rgb"), Some(&json!([1, 2, 3])));
    assert_eq!(
        driver_impl.commands(),
        vec![LightCommand::SetRgb { rgb: Rgb { r: 1, g: 2, b: 3 }, transition_ms: Some(1000) }]
    );

    Ok(())
}
