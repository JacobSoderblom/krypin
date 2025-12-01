use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::Utc;
use hub_core::{
    bus::{Bus, InMemoryBus},
    bus_contract::{
        CommandSet, DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX,
        TOPIC_DEVICE_ANNOUNCE, TOPIC_ENTITY_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    model::{Device, DeviceId, Entity, EntityDomain, EntityId, EntityState},
    storage::Storage,
};
use hubd::{config::AuthConfig, http::build_router, state::AppState, subscribers};
use tokio::{
    sync::oneshot,
    time::{Duration, sleep, timeout},
};
use tokio_stream::StreamExt;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default, Clone)]
struct HarnessStorage {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    areas: HashMap<hub_core::model::AreaId, hub_core::model::Area>,
    devices: HashMap<DeviceId, Device>,
    entities: HashMap<EntityId, Entity>,
    states: HashMap<EntityId, Vec<EntityState>>,
}

#[async_trait]
impl Storage for HarnessStorage {
    async fn list_areas(&self) -> Result<Vec<hub_core::model::Area>> {
        Ok(self.inner.lock().unwrap().areas.values().cloned().collect())
    }

    async fn upsert_area(&self, area: hub_core::model::Area) -> Result<()> {
        self.inner.lock().unwrap().areas.insert(area.id, area);
        Ok(())
    }

    async fn get_area(&self, id: hub_core::model::AreaId) -> Result<Option<hub_core::model::Area>> {
        Ok(self.inner.lock().unwrap().areas.get(&id).cloned())
    }

    async fn list_devices(&self) -> Result<Vec<Device>> {
        Ok(self.inner.lock().unwrap().devices.values().cloned().collect())
    }

    async fn upsert_device(&self, device: Device) -> Result<()> {
        self.inner.lock().unwrap().devices.insert(device.id, device);
        Ok(())
    }

    async fn get_device(&self, id: DeviceId) -> Result<Option<Device>> {
        Ok(self.inner.lock().unwrap().devices.get(&id).cloned())
    }

    async fn list_entities(&self) -> Result<Vec<Entity>> {
        Ok(self.inner.lock().unwrap().entities.values().cloned().collect())
    }

    async fn upsert_entity(&self, entity: Entity) -> Result<()> {
        if !self.inner.lock().unwrap().devices.contains_key(&entity.device_id) {
            return Err(anyhow!("device missing for entity"));
        }
        self.inner.lock().unwrap().entities.insert(entity.id, entity);
        Ok(())
    }

    async fn get_entity(&self, id: EntityId) -> Result<Option<Entity>> {
        Ok(self.inner.lock().unwrap().entities.get(&id).cloned())
    }

    async fn set_entity_state(&self, state: EntityState) -> Result<()> {
        let mut guard = self.inner.lock().unwrap();
        if !guard.entities.contains_key(&state.entity_id) {
            return Err(anyhow!("entity not found"));
        }
        guard.states.entry(state.entity_id).or_default().push(state);
        Ok(())
    }

    async fn latest_entity_state(&self, id: EntityId) -> Result<Option<EntityState>> {
        Ok(self.inner.lock().unwrap().states.get(&id).and_then(|v| v.last().cloned()))
    }

    async fn entity_state_history(
        &self,
        id: EntityId,
        _since: Option<chrono::DateTime<chrono::Utc>>,
        _limit: usize,
    ) -> Result<Vec<EntityState>> {
        Ok(self.inner.lock().unwrap().states.get(&id).cloned().unwrap_or_default())
    }
}

#[tokio::test]
async fn hub_round_trips_commands_and_state_updates() -> Result<()> {
    let bus = Arc::new(InMemoryBus::default());
    let store = Arc::new(HarnessStorage::default());
    let app_state =
        AppState { store: store.clone(), bus: bus.clone(), auth: AuthConfig::default() };

    subscribers::spawn_all(app_state.clone());
    sleep(Duration::from_millis(50)).await;

    let device_id = DeviceId(Uuid::new_v4());
    let entity_id = EntityId(Uuid::new_v4());

    let device = DeviceAnnounce {
        id: device_id,
        name: "Test Device".into(),
        adapter: "mock".into(),
        manufacturer: None,
        model: None,
        sw_version: None,
        hw_version: None,
        area: None,
        metadata: Default::default(),
    };
    bus.publish(TOPIC_DEVICE_ANNOUNCE, serde_json::to_vec(&device)?.into()).await?;
    timeout(Duration::from_secs(2), async {
        loop {
            if store.get_device(device_id).await?.is_some() {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;

    let entity = EntityAnnounce {
        id: entity_id,
        device_id,
        name: "Test Switch".into(),
        domain: EntityDomain::Switch,
        icon: None,
        key: None,
        attributes: Default::default(),
    };
    bus.publish(TOPIC_ENTITY_ANNOUNCE, serde_json::to_vec(&entity)?.into()).await?;
    timeout(Duration::from_secs(2), async {
        loop {
            if store.get_entity(entity_id).await?.is_some() {
                return Ok::<(), anyhow::Error>(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;

    let (cmd_tx, cmd_rx) = oneshot::channel();
    let (state_tx, state_rx) = oneshot::channel();
    let mut adapter_stream =
        bus.subscribe(&format!("{TOPIC_COMMAND_PREFIX}{}", entity_id.0)).await?;
    let adapter_bus = bus.clone();
    tokio::spawn(async move {
        if let Some(msg) = adapter_stream.next().await {
            let cmd: CommandSet = serde_json::from_slice(&msg.payload).expect("command payload");
            let _ = cmd_tx.send(cmd.clone());

            let update = StateUpdate {
                entity_id,
                value: serde_json::json!({"reported": cmd.value}),
                attributes: Default::default(),
                ts: Utc::now(),
                source: Some("mock-adapter".into()),
            };
            let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", entity_id.0);
            let payload = serde_json::to_vec(&update).expect("state update serialization");
            let _ = adapter_bus.publish(&topic, payload.into()).await;
            let _ = state_tx.send(());
        }
    });

    let app = build_router(app_state);
    let response = app
        .oneshot(
            Request::post(format!("/command/{}", entity_id.0))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"action":"set","value":true}"#))
                .unwrap(),
        )
        .await?;
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let received = timeout(Duration::from_secs(2), cmd_rx).await.expect("adapter command recv")?;
    assert_eq!(received.value, serde_json::json!(true));

    let _ = timeout(Duration::from_secs(2), state_rx).await.expect("state publish");

    timeout(Duration::from_secs(2), async {
        loop {
            if let Some(state) = store.latest_entity_state(entity_id).await.unwrap() {
                assert_eq!(state.value["reported"], serde_json::json!(true));
                assert_eq!(state.source.as_deref(), Some("mock-adapter"));
                break;
            }
            sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("state stored");

    Ok(())
}
