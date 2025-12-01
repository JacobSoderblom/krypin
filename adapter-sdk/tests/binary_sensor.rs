use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use adapter_sdk::{
    light::{DeviceMeta, EntityMeta},
    sensor::{BinarySensorComponent, BinarySensorDriver},
};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{StateUpdate, TOPIC_DEVICE_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX},
    cap::sensor::{BinarySensorDescription, BinarySensorDeviceClass, BinarySensorState},
    model::{DeviceId, EntityId},
};
use tokio::sync::broadcast;
use tokio::time::{Duration, timeout};
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use uuid::Uuid;

#[derive(Clone)]
struct TestDriver {
    description: BinarySensorDescription,
    state: Arc<Mutex<BinarySensorState>>,
    tx: broadcast::Sender<BinarySensorState>,
}

impl TestDriver {
    fn new(description: BinarySensorDescription, state: BinarySensorState) -> Self {
        let (tx, _rx) = broadcast::channel(16);
        Self { description, state: Arc::new(Mutex::new(state)), tx }
    }

    fn send_state(&self, state: BinarySensorState) {
        *self.state.lock().unwrap() = state.clone();
        let _ = self.tx.send(state);
    }
}

#[async_trait]
impl BinarySensorDriver for TestDriver {
    fn describe(&self) -> BinarySensorDescription {
        self.description.clone()
    }

    async fn current_state(&self) -> Result<BinarySensorState> {
        Ok(self.state.lock().unwrap().clone())
    }

    fn updates(
        &self,
    ) -> Result<Box<dyn tokio_stream::Stream<Item = Result<BinarySensorState>> + Send + Unpin>>
    {
        let rx = self.tx.subscribe();
        let stream = BroadcastStream::new(rx).map(|item| match item {
            Ok(state) => Ok(state),
            Err(err) => Err(anyhow!(err)),
        });
        Ok(Box::new(stream))
    }
}

fn make_device_meta(device_id: DeviceId) -> DeviceMeta {
    DeviceMeta {
        id: device_id,
        name: "Binary Sensor".into(),
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
        name: "Binary Sensor".into(),
        icon: None,
        attributes: BTreeMap::new(),
    }
}

#[tokio::test]
async fn publishes_initial_and_subsequent_updates() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = BinarySensorDescription {
        entity_id,
        device_class: Some(BinarySensorDeviceClass::Door),
        inverted: false,
    };
    let initial_state = BinarySensorState { on: false };
    let driver_impl = Arc::new(TestDriver::new(description, initial_state));
    let component = BinarySensorComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;
    let mut announce_sub = bus_impl.subscribe(TOPIC_DEVICE_ANNOUNCE).await?;

    component.clone().spawn().await?;

    // consume the two announce messages
    for _ in 0..2 {
        let _ = timeout(Duration::from_millis(200), announce_sub.next())
            .await
            .expect("announce not received")
            .expect("announce stream closed unexpectedly");
    }

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("initial state not received")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.entity_id, entity_id);
    assert_eq!(update.value, serde_json::Value::Bool(false));
    assert_eq!(update.attributes.get("device_class").and_then(|v| v.as_str()), Some("door"));

    driver_impl.send_state(BinarySensorState { on: true });

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("update not received")
        .expect("state stream closed unexpectedly");

    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.value, serde_json::Value::Bool(true));

    Ok(())
}

#[tokio::test]
async fn inverted_sensor_flips_reported_state() -> Result<()> {
    let bus_impl = Arc::new(InMemoryBus::default());
    let bus: Arc<dyn Bus> = bus_impl.clone();

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = make_device_meta(device_id);
    let entity = make_entity_meta(entity_id);

    let description = BinarySensorDescription { entity_id, device_class: None, inverted: true };
    let initial_state = BinarySensorState { on: false };
    let driver_impl = Arc::new(TestDriver::new(description, initial_state));
    let component = BinarySensorComponent::new(bus, device, entity, driver_impl.clone());

    let mut state_sub =
        bus_impl.subscribe(&format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0)).await?;

    component.clone().spawn().await?;

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("initial state not received")
        .expect("state stream closed unexpectedly");
    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.value, serde_json::Value::Bool(true));
    assert_eq!(update.attributes.get("inverted").and_then(|v| v.as_bool()), Some(true));

    driver_impl.send_state(BinarySensorState { on: true });

    let message = timeout(Duration::from_millis(200), state_sub.next())
        .await
        .expect("update not received")
        .expect("state stream closed unexpectedly");
    let update: StateUpdate = serde_json::from_slice(&message.payload)?;
    assert_eq!(update.value, serde_json::Value::Bool(false));

    Ok(())
}
