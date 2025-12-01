use std::{collections::BTreeMap, sync::Arc};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use hub_core::{
    bus::Bus,
    bus_contract::{
        DeviceAnnounce, EntityAnnounce, StateUpdate, TOPIC_COMMAND_PREFIX, TOPIC_DEVICE_ANNOUNCE,
        TOPIC_STATE_UPDATE_PREFIX,
    },
    cap::robotvac::{RobotVacCommand, RobotVacDescription, RobotVacState, RobotVacStatus},
    model::{EntityDomain, EntityId},
};
use tokio_stream::StreamExt;

pub use crate::light::{DeviceMeta, EntityMeta};

#[async_trait]
pub trait RobotVacDriver: Send + Sync + 'static {
    fn describe(&self) -> RobotVacDescription;
    async fn apply_command(&self, cmd: RobotVacCommand) -> Result<RobotVacState>;
    async fn refresh(&self) -> Result<RobotVacState> {
        Err(anyhow!("refresh not implemented"))
    }
}

#[derive(Clone)]
pub struct RobotVacComponent {
    bus: Arc<dyn Bus>,
    device: DeviceMeta,
    entity: EntityMeta,
    driver: Arc<dyn RobotVacDriver>,
}

impl RobotVacComponent {
    pub fn new(
        bus: Arc<dyn Bus>,
        device: DeviceMeta,
        entity: EntityMeta,
        driver: Arc<dyn RobotVacDriver>,
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
                    tracing::warn!("robot vac component command error: {err}");
                }
            }
        });
        Ok(())
    }

    pub async fn publish_state(&self, state: RobotVacState) -> Result<()> {
        let update = RobotVacStateMapper::to_state_update(self.entity.id, state);
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (self.entity.id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await?;
        Ok(())
    }

    async fn handle_command_bytes(&self, bytes: &[u8]) -> Result<()> {
        let raw: serde_json::Value = serde_json::from_slice(bytes).context("parse command json")?;
        let command = RobotVacCommandMapper::from_json(&raw)?;
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
            domain: EntityDomain::RobotVacuum,
            icon: entity.icon.clone(),
            key: None,
            attributes: entity.attributes.clone(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }
}

struct RobotVacCommandMapper;
impl RobotVacCommandMapper {
    pub fn from_json(v: &serde_json::Value) -> Result<RobotVacCommand> {
        let action = v.get("action").and_then(|a| a.as_str()).unwrap_or("start");
        match action {
            "start" => Ok(RobotVacCommand::Start),
            "pause" => Ok(RobotVacCommand::Pause),
            "stop" => Ok(RobotVacCommand::Stop),
            "dock" => Ok(RobotVacCommand::Dock),
            "locate" => Ok(RobotVacCommand::Locate),
            "spot_clean" => Ok(RobotVacCommand::SpotClean),
            _ => Err(anyhow!("unsupported robot vacuum command")),
        }
    }
}

struct RobotVacStateMapper;
impl RobotVacStateMapper {
    pub fn to_state_update(entity_id: EntityId, state: RobotVacState) -> StateUpdate {
        let mut attributes = BTreeMap::new();
        if let Some(battery) = state.battery_level {
            attributes.insert("battery".into(), serde_json::Value::from(battery as u64));
        }
        if let Some(fan_power) = state.fan_power {
            attributes.insert("fan_power".into(), serde_json::Value::from(fan_power as u64));
        }

        StateUpdate {
            entity_id,
            value: serde_json::Value::String(
                match state.status {
                    RobotVacStatus::Idle => "idle",
                    RobotVacStatus::Cleaning => "cleaning",
                    RobotVacStatus::Paused => "paused",
                    RobotVacStatus::Returning => "returning",
                    RobotVacStatus::Docked => "docked",
                    RobotVacStatus::Error => "error",
                }
                .into(),
            ),
            attributes,
            ts: Utc::now(),
            source: Some("adapter-sdk:robotvac".into()),
        }
    }
}
