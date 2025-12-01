use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hub_core::{
    bus::Bus,
    bus_contract::{
        DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE,
        TOPIC_ENTITY_ANNOUNCE, TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::switch::{SwitchCommand, SwitchDescription, SwitchState},
    model::{EntityDomain, EntityId},
};
use tokio_stream::StreamExt;

use crate::meta::{DeviceMeta, EntityMeta};

#[async_trait]
pub trait SwitchDriver: Send + Sync + 'static {
    fn describe(&self) -> SwitchDescription;
    async fn apply_command(&self, cmd: SwitchCommand) -> Result<SwitchState>;
    async fn refresh(&self) -> Result<SwitchState> {
        Err(anyhow!("refresh not implemented"))
    }
}

#[derive(Clone)]
pub struct SwitchComponent {
    bus: Arc<dyn Bus>,
    device: DeviceMeta,
    entity: EntityMeta,
    driver: Arc<dyn SwitchDriver>,
}

impl SwitchComponent {
    pub fn new(
        bus: Arc<dyn Bus>,
        device: DeviceMeta,
        entity: EntityMeta,
        driver: Arc<dyn SwitchDriver>,
    ) -> Self {
        Self { bus, device, entity, driver }
    }

    pub async fn spawn(self) -> Result<()> {
        self.announce_device().await?;
        self.announce_entity().await?;

        let topic = format!("{TOPIC_COMMAND_PREFIX}{}", (self.entity.id).0);
        let mut stream = self.bus.subscribe(&topic).await?;
        let this = self.clone();

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                if let Err(err) = this.handle_command_bytes(&msg.payload).await {
                    tracing::warn!("switch component command error: {err}");
                }
            }
        });

        Ok(())
    }

    pub async fn publish_state(&self, state: SwitchState) -> Result<()> {
        let update = SwitchStateMapper::to_state_update(self.entity.id, state);
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (self.entity.id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await?;
        Ok(())
    }

    async fn handle_command_bytes(&self, bytes: &[u8]) -> Result<()> {
        let raw: serde_json::Value = serde_json::from_slice(bytes).context("parse command json")?;
        let command = SwitchCommandMapper::from_json(&raw)?;
        let description = self.driver.describe();
        description.validate(&command).map_err(|e| anyhow!(e))?;
        let state = self.driver.apply_command(command).await?;
        self.publish_state(state).await?;
        Ok(())
    }

    async fn announce_device(&self) -> Result<()> {
        let device = &self.device;
        let message = DeviceAnnounce {
            id: device.id,
            name: device.name.clone(),
            adapter: device.adapter.clone(),
            manufacturer: device.manufacturer.clone(),
            model: device.model.clone(),
            sw_version: device.sw_version.clone(),
            hw_version: device.hw_version.clone(),
            area: device.area,
            metadata: device.metadata_map(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }

    async fn announce_entity(&self) -> Result<()> {
        let entity = &self.entity;
        let message = EntityAnnounce {
            id: entity.id,
            device_id: self.device.id,
            name: entity.name.clone(),
            domain: EntityDomain::Switch,
            icon: entity.icon.clone(),
            key: None,
            attributes: entity.attributes.clone(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        self.bus.publish(TOPIC_ENTITY_ANNOUNCE, bytes).await?;
        Ok(())
    }
}

struct SwitchCommandMapper;

impl SwitchCommandMapper {
    fn from_json(value: &serde_json::Value) -> Result<SwitchCommand> {
        let action = value.get("action").and_then(|v| v.as_str()).unwrap_or("set");
        let raw_value = value.get("value");
        match action {
            "toggle" => Ok(SwitchCommand::Toggle),
            _ => {
                if let Some(on) = value.get("on").and_then(|v| v.as_bool()) {
                    return Ok(SwitchCommand::Set { on });
                }
                if let Some(on) = raw_value
                    .and_then(|v| v.as_object())
                    .and_then(|obj| obj.get("on"))
                    .and_then(|v| v.as_bool())
                {
                    return Ok(SwitchCommand::Set { on });
                }
                if let Some(on) = raw_value.and_then(|v| v.as_bool()) {
                    return Ok(SwitchCommand::Set { on });
                }
                Err(anyhow!("unsupported switch command payload"))
            }
        }
    }
}

struct SwitchStateMapper;

impl SwitchStateMapper {
    fn to_state_update(entity_id: EntityId, state: SwitchState) -> StateUpdate {
        let mut attributes = BTreeMap::new();
        if let Some(power) = state.power_w {
            attributes.insert("power_w".into(), serde_json::Value::from(power as f64));
        }
        StateUpdate {
            entity_id,
            value: serde_json::Value::Bool(state.on),
            attributes,
            ts: Utc::now(),
            source: Some("adapter-sdk:switch".into()),
        }
    }
}
