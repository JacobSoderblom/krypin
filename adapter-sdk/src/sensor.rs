use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hub_core::{
    bus::Bus,
    bus_contract::{
        DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_DEVICE_ANNOUNCE,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::sensor::{BinarySensorDescription, BinarySensorState},
    model::{EntityDomain, EntityId},
};
use tokio_stream::{Stream, StreamExt};

use crate::light::{DeviceMeta, EntityMeta};

#[async_trait]
pub trait BinarySensorDriver: Send + Sync + 'static {
    fn describe(&self) -> BinarySensorDescription;
    async fn current_state(&self) -> Result<BinarySensorState>;
    fn updates(&self) -> Result<Box<dyn Stream<Item = Result<BinarySensorState>> + Send + Unpin>>;
    async fn refresh(&self) -> Result<BinarySensorState> {
        self.current_state().await
    }
}

#[derive(Clone)]
pub struct BinarySensorComponent {
    bus: Arc<dyn Bus>,
    device: DeviceMeta,
    entity: EntityMeta,
    driver: Arc<dyn BinarySensorDriver>,
}

impl BinarySensorComponent {
    pub fn new(
        bus: Arc<dyn Bus>,
        device: DeviceMeta,
        entity: EntityMeta,
        driver: Arc<dyn BinarySensorDriver>,
    ) -> Self {
        Self { bus, device, entity, driver }
    }

    pub async fn spawn(self) -> Result<()> {
        self.announce_device().await?;
        self.announce_entity().await?;

        let initial = self.driver.current_state().await?;
        self.publish_state(initial).await?;

        let mut updates = self.driver.updates()?;
        let this = self.clone();

        tokio::spawn(async move {
            while let Some(next) = updates.next().await {
                match next {
                    Ok(state) => {
                        if let Err(err) = this.publish_state(state).await {
                            tracing::warn!("binary sensor publish error: {err}");
                        }
                    }
                    Err(err) => {
                        tracing::warn!("binary sensor driver update error: {err}");
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn publish_state(&self, state: BinarySensorState) -> Result<()> {
        let description = self.driver.describe();
        let update = BinarySensorStateMapper::to_state_update(self.entity.id, &description, state);
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (self.entity.id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await?;
        Ok(())
    }

    async fn announce_device(&self) -> Result<()> {
        let d = &self.device;
        let msg = DeviceAnnounce {
            id: d.id,
            name: d.name.clone(),
            adapter: d.adapter.clone(),
            manufacturer: d.manufacturer.clone(),
            model: d.model.clone(),
            sw_version: d.sw_version.clone(),
            hw_version: d.hw_version.clone(),
            area: d.area,
            metadata: d.metadata_map(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&msg)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }

    async fn announce_entity(&self) -> Result<()> {
        let e = &self.entity;
        let msg = EntityAnnounce {
            id: e.id,
            device_id: self.device.id,
            name: e.name.clone(),
            domain: EntityDomain::BinarySensor,
            icon: e.icon.clone(),
            key: None,
            attributes: e.attributes.clone(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&msg)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }
}

struct BinarySensorStateMapper;

impl BinarySensorStateMapper {
    fn to_state_update(
        entity_id: EntityId,
        description: &BinarySensorDescription,
        state: BinarySensorState,
    ) -> StateUpdate {
        let mut attrs = BTreeMap::new();
        if let Some(class) = description.device_class {
            attrs.insert(
                "device_class".into(),
                serde_json::Value::String(class.as_str().to_string()),
            );
        }
        if description.inverted {
            attrs.insert("inverted".into(), serde_json::Value::Bool(true));
        }

        let effective = if description.inverted { !state.on } else { state.on };

        StateUpdate {
            entity_id,
            value: serde_json::Value::Bool(effective),
            attributes: attrs,
            ts: Utc::now(),
            source: Some("adapter-sdk:binary-sensor".into()),
        }
    }
}
