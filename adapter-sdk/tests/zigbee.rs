use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::zigbee::ZigbeeInfo;
use adapter_sdk::{
    light::{LightComponent, LightDriver},
    meta::{DeviceMeta, EntityMeta},
};
use anyhow::Result;
use async_trait::async_trait;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        DeviceAnnounce, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE,
    },
    cap::light::{LightCommand, LightDescription, LightFeatures, LightState, Power},
    model::{DeviceId, EntityId},
};
use tokio::time::{Duration, timeout};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[test]
fn zigbee_info_builder_sets_all_fields() {
    let zigbee = ZigbeeInfo::new("0x00124b0001abcdef")
        .with_network_address(0x1a2b)
        .with_endpoints(vec![1, 3, 5])
        .with_power_source("battery")
        .with_firmware_version("2.0.1");

    assert_eq!(zigbee.ieee_address, "0x00124b0001abcdef");
    assert_eq!(zigbee.network_address, Some(0x1a2b));
    assert_eq!(zigbee.endpoints, Some(vec![1, 3, 5]));
    assert_eq!(zigbee.power_source.as_deref(), Some("battery"));
    assert_eq!(zigbee.firmware_version.as_deref(), Some("2.0.1"));
}

#[test]
fn zigbee_info_serializes_without_empty_fields() {
    let zigbee = ZigbeeInfo::new("0x00124b0001abcdef");
    let value = serde_json::to_value(&zigbee).expect("serialize zigbee");

    assert_eq!(value["ieee_address"], "0x00124b0001abcdef");
    assert!(value.get("network_address").is_none());
    assert!(value.get("endpoints").is_none());
    assert!(value.get("power_source").is_none());
    assert!(value.get("firmware_version").is_none());
}

#[test]
fn device_metadata_map_merges_zigbee() {
    let mut metadata = BTreeMap::new();
    metadata.insert("existing".into(), serde_json::json!({"k": "v"}));

    let zigbee = ZigbeeInfo::new("0x00124b0001abcdef");
    let device = make_device_meta(DeviceId(Uuid::nil()), zigbee.clone());

    let merged = DeviceMeta { metadata, ..device }.metadata_map();

    assert!(merged.contains_key("existing"));
    assert_eq!(merged.get("zigbee").cloned(), serde_json::to_value(zigbee).ok());
}

struct NoopDriver {
    description: LightDescription,
    state: Arc<Mutex<LightState>>,
}

impl NoopDriver {
    fn new(description: LightDescription, state: LightState) -> Self {
        Self { description, state: Arc::new(Mutex::new(state)) }
    }
}

#[async_trait]
impl LightDriver for NoopDriver {
    fn describe(&self) -> LightDescription {
        self.description.clone()
    }

    async fn apply_command(&self, cmd: LightCommand) -> Result<LightState> {
        if let LightCommand::SetPower { on } = cmd {
            let mut g = self.state.lock().unwrap();
            g.power = if on { Power::On } else { Power::Off };
        }
        Ok(self.state.lock().unwrap().clone())
    }
}

fn make_device_meta(device_id: DeviceId, zigbee: ZigbeeInfo) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Zigbee Light".into(),
        adapter: "test-adapter".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: BTreeMap::new(),
        zigbee: Some(zigbee),
    }
}

fn make_entity_meta(entity_id: EntityId) -> EntityMeta {
    EntityMeta {
        id: entity_id,
        name: "Zigbee Light".into(),
        icon: None,
        attributes: BTreeMap::new(),
    }
}

#[tokio::test]
async fn announces_zigbee_metadata_on_device() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let zigbee = ZigbeeInfo::new("0x00124b0026abc123")
        .with_network_address(0x1234)
        .with_endpoints(vec![1, 11])
        .with_power_source("mains")
        .with_firmware_version("1.0.0");

    let device = make_device_meta(device_id, zigbee.clone());
    let entity = make_entity_meta(entity_id);

    let description = LightDescription {
        entity_id,
        features: LightFeatures::ONOFF,
        min_mireds: None,
        max_mireds: None,
    };
    let driver = Arc::new(NoopDriver::new(
        description,
        LightState { power: Power::Off, brightness: None, color: None },
    ));

    let component = LightComponent::new(bus.clone(), device, entity, driver);

    let mut device_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;
    let mut entity_sub = bus_impl.subscribe(TOPIC_ENTITY_ANNOUNCE).await?;

    component.clone().spawn().await?;

    let mut received = None;
    let msg = timeout(Duration::from_millis(200), device_sub.next())
        .await
        .expect("announce not received")
        .expect("announce stream closed unexpectedly");

    if let Ok(announce) = serde_json::from_slice::<DeviceAnnounce>(&msg.payload) {
        received = Some(announce.metadata);
    }

    timeout(Duration::from_millis(200), entity_sub.next())
        .await
        .expect("entity announce not received")
        .expect("entity stream closed unexpectedly");

    let metadata = received.expect("device announce not found");
    let zigbee_value = metadata.get("zigbee").expect("missing zigbee metadata").clone();
    let parsed: ZigbeeInfo = serde_json::from_value(zigbee_value)?;
    assert_eq!(parsed, zigbee);

    // ensure command subscription still works with zigbee metadata present
    let command_topic = format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0);
    bus.publish(&command_topic, serde_json::to_vec(&LightCommand::SetPower { on: true })?.into())
        .await?;

    Ok(())
}
