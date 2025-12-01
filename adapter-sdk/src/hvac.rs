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
    cap::hvac::{HvacCommand, HvacDescription, HvacFanMode, HvacMode, HvacState, Temperature},
    model::{EntityDomain, EntityId},
};
use tokio_stream::StreamExt;

pub use crate::light::{DeviceMeta, EntityMeta};

#[async_trait]
pub trait HvacDriver: Send + Sync + 'static {
    fn describe(&self) -> HvacDescription;
    async fn apply_command(&self, cmd: HvacCommand) -> Result<HvacState>;
    async fn refresh(&self) -> Result<HvacState> {
        Err(anyhow!("refresh not implemented"))
    }
}

#[derive(Clone)]
pub struct HvacComponent {
    bus: Arc<dyn Bus>,
    device: DeviceMeta,
    entity: EntityMeta,
    driver: Arc<dyn HvacDriver>,
}

impl HvacComponent {
    pub fn new(
        bus: Arc<dyn Bus>,
        device: DeviceMeta,
        entity: EntityMeta,
        driver: Arc<dyn HvacDriver>,
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
                    tracing::warn!("hvac component command error: {err}");
                }
            }
        });
        Ok(())
    }

    pub async fn publish_state(&self, state: HvacState) -> Result<()> {
        let update = HvacStateMapper::to_state_update(self.entity.id, state);
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (self.entity.id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await?;
        Ok(())
    }

    async fn handle_command_bytes(&self, bytes: &[u8]) -> Result<()> {
        let raw: serde_json::Value = serde_json::from_slice(bytes).context("parse command json")?;
        let command = HvacCommandMapper::from_json(&raw)?;
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
            domain: EntityDomain::Climate,
            icon: entity.icon.clone(),
            key: None,
            attributes: entity.attributes.clone(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&message)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }
}

struct HvacCommandMapper;
impl HvacCommandMapper {
    fn parse_mode(v: &serde_json::Value) -> Option<HvacMode> {
        v.as_str().and_then(|s| match s {
            "heat" => Some(HvacMode::Heat),
            "cool" => Some(HvacMode::Cool),
            "auto" => Some(HvacMode::Auto),
            "dry" => Some(HvacMode::Dry),
            "fan_only" => Some(HvacMode::FanOnly),
            "off" => Some(HvacMode::Off),
            _ => None,
        })
    }

    fn parse_fan_mode(v: &serde_json::Value) -> Option<HvacFanMode> {
        v.as_str().and_then(|s| match s {
            "auto" => Some(HvacFanMode::Auto),
            "low" => Some(HvacFanMode::Low),
            "medium" => Some(HvacFanMode::Medium),
            "high" => Some(HvacFanMode::High),
            "turbo" => Some(HvacFanMode::Turbo),
            "quiet" => Some(HvacFanMode::Quiet),
            _ => None,
        })
    }

    pub fn from_json(v: &serde_json::Value) -> Result<HvacCommand> {
        let action = v.get("action").and_then(|a| a.as_str()).unwrap_or("set");
        let value = v.get("value").unwrap_or(v);

        match action {
            "set_mode" => {
                let mode = value
                    .get("mode")
                    .and_then(Self::parse_mode)
                    .or_else(|| Self::parse_mode(value))
                    .ok_or_else(|| anyhow!("missing hvac mode"))?;
                Ok(HvacCommand::SetMode { mode })
            }
            "set_temperature" => {
                let temp = value
                    .get("target_temperature_c")
                    .or_else(|| value.get("temperature"))
                    .and_then(|v| v.as_f64())
                    .map(|v| Temperature(v as f32))
                    .ok_or_else(|| anyhow!("missing target temperature"))?;
                Ok(HvacCommand::SetTargetTemperature { temperature: temp })
            }
            "set_fan_mode" => {
                let fan_mode = value
                    .get("fan_mode")
                    .and_then(Self::parse_fan_mode)
                    .or_else(|| Self::parse_fan_mode(value))
                    .ok_or_else(|| anyhow!("missing fan mode"))?;
                Ok(HvacCommand::SetFanMode { fan_mode })
            }
            _ => {
                if let Some(mode) = value.get("mode").and_then(Self::parse_mode) {
                    return Ok(HvacCommand::SetMode { mode });
                }
                if let Some(temp) = value
                    .get("target_temperature_c")
                    .or_else(|| value.get("temperature"))
                    .and_then(|v| v.as_f64())
                {
                    return Ok(HvacCommand::SetTargetTemperature {
                        temperature: Temperature(temp as f32),
                    });
                }
                if let Some(fan) = value.get("fan_mode").and_then(Self::parse_fan_mode) {
                    return Ok(HvacCommand::SetFanMode { fan_mode: fan });
                }
                Err(anyhow!("unsupported hvac command payload"))
            }
        }
    }
}

struct HvacStateMapper;
impl HvacStateMapper {
    pub fn to_state_update(entity_id: EntityId, state: HvacState) -> StateUpdate {
        let mut attributes = BTreeMap::new();
        if let Some(temp) = state.target_temperature {
            attributes.insert("target_temperature_c".into(), serde_json::Value::from(temp.0));
        }
        if let Some(temp) = state.ambient_temperature {
            attributes.insert("ambient_temperature_c".into(), serde_json::Value::from(temp.0));
        }
        if let Some(fan) = state.fan_mode {
            let value = match fan {
                HvacFanMode::Auto => "auto",
                HvacFanMode::Low => "low",
                HvacFanMode::Medium => "medium",
                HvacFanMode::High => "high",
                HvacFanMode::Turbo => "turbo",
                HvacFanMode::Quiet => "quiet",
            };
            attributes.insert("fan_mode".into(), serde_json::Value::String(value.into()));
        }

        StateUpdate {
            entity_id,
            value: serde_json::Value::String(
                match state.mode {
                    HvacMode::Off => "off",
                    HvacMode::Heat => "heat",
                    HvacMode::Cool => "cool",
                    HvacMode::Auto => "auto",
                    HvacMode::Dry => "dry",
                    HvacMode::FanOnly => "fan_only",
                }
                .into(),
            ),
            attributes,
            ts: Utc::now(),
            source: Some("adapter-sdk:hvac".into()),
        }
    }
}
