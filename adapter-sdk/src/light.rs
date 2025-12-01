use std::{collections::BTreeMap, sync::Arc};

use crate::zigbee::ZigbeeInfo;
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
    cap::light::{Brightness, LightCommand, LightDescription, LightState, Mireds, Rgb},
    model::{DeviceId, EntityDomain, EntityId},
};
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMeta {
    pub id: DeviceId,
    pub name: String,
    pub adapter: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub sw_version: Option<String>,
    pub hw_version: Option<String>,
    pub area: Option<Uuid>,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub zigbee: Option<ZigbeeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMeta {
    pub id: EntityId,
    pub name: String,
    pub icon: Option<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl DeviceMeta {
    pub fn metadata_map(&self) -> BTreeMap<String, serde_json::Value> {
        let mut metadata = self.metadata.clone();
        if let Some(zigbee) = &self.zigbee
            && let Ok(value) = serde_json::to_value(zigbee)
        {
            metadata.insert("zigbee".into(), value);
        }
        metadata
    }
}

#[async_trait]
pub trait LightDriver: Send + Sync + 'static {
    fn describe(&self) -> LightDescription;
    async fn apply_command(&self, cmd: LightCommand) -> Result<LightState>;
    async fn refresh(&self) -> Result<LightState> {
        Err(anyhow!("refresh not implemented"))
    }
}

#[derive(Clone)]
pub struct LightComponent {
    bus: Arc<dyn Bus>,
    device: DeviceMeta,
    entity: EntityMeta,
    driver: Arc<dyn LightDriver>,
}

impl LightComponent {
    pub fn new(
        bus: Arc<dyn Bus>,
        device: DeviceMeta,
        entity: EntityMeta,
        driver: Arc<dyn LightDriver>,
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
                if let Err(e) = this.handle_command_bytes(&msg.payload).await {
                    tracing::warn!("light component command error: {e}");
                }
            }
        });
        Ok(())
    }

    pub async fn publish_state(&self, st: LightState) -> Result<()> {
        let update = LightStateMapper::to_state_update(self.entity.id, st);
        let topic = format!("{TOPIC_STATE_UPDATE_PREFIX}{}", (self.entity.id).0);
        let bytes = Bytes::from(serde_json::to_vec(&update)?);
        self.bus.publish(&topic, bytes).await?;
        Ok(())
    }

    async fn handle_command_bytes(&self, bytes: &[u8]) -> Result<()> {
        let raw: serde_json::Value = serde_json::from_slice(bytes).context("parse command json")?;
        let cmd = LightCommandMapper::from_json(&raw)?;
        let desc = self.driver.describe();
        desc.validate(&cmd).map_err(|e| anyhow!(e))?;
        let state = self.driver.apply_command(cmd).await?;
        self.publish_state(state).await?;
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
            domain: EntityDomain::Light,
            icon: e.icon.clone(),
            key: None,
            attributes: e.attributes.clone(),
        };
        let bytes = Bytes::from(serde_json::to_vec(&msg)?);
        self.bus.publish(TOPIC_DEVICE_ANNOUNCE, bytes).await?;
        Ok(())
    }
}

struct LightCommandMapper;
impl LightCommandMapper {
    pub fn from_json(v: &serde_json::Value) -> Result<LightCommand> {
        let action = v.get("action").and_then(|s| s.as_str()).unwrap_or("set");
        let val = v.get("value");
        match action {
            "toggle" => Ok(LightCommand::Toggle),
            _ => {
                if let Some(obj) = val.and_then(|x| x.as_object()) {
                    if let Some(b) = obj.get("on").and_then(|x| x.as_bool()) {
                        return Ok(LightCommand::SetPower { on: b });
                    }
                    if let Some(n) = obj.get("brightness").and_then(|x| x.as_u64()) {
                        let level = u8::try_from(n).unwrap_or(100).min(100);
                        let trans =
                            obj.get("transition_ms").and_then(|x| x.as_u64()).map(|n| n as u32);
                        return Ok(LightCommand::SetBrightness {
                            level: Brightness(level),
                            transition_ms: trans,
                        });
                    }
                    if let Some(n) = obj.get("mireds").and_then(|x| x.as_u64()) {
                        let m = Mireds(n as u16);
                        let trans =
                            obj.get("transition_ms").and_then(|x| x.as_u64()).map(|n| n as u32);
                        return Ok(LightCommand::SetColorTemp { mireds: m, transition_ms: trans });
                    }
                    if let Some(rgb) =
                        obj.get("rgb").and_then(|x| x.as_array()).filter(|a| a.len() == 3)
                    {
                        let r = rgb[0].as_u64().unwrap_or(0) as u8;
                        let g = rgb[1].as_u64().unwrap_or(0) as u8;
                        let b = rgb[2].as_u64().unwrap_or(0) as u8;
                        let trans =
                            obj.get("transition_ms").and_then(|x| x.as_u64()).map(|n| n as u32);
                        return Ok(LightCommand::SetRgb {
                            rgb: Rgb { r, g, b },
                            transition_ms: trans,
                        });
                    }
                }
                if let Some(b) = val.and_then(|x| x.as_bool()) {
                    return Ok(LightCommand::SetPower { on: b });
                }
                Err(anyhow!("unsupported light command payload"))
            }
        }
    }
}

struct LightStateMapper;
impl LightStateMapper {
    pub fn to_state_update(entity_id: EntityId, st: LightState) -> StateUpdate {
        let mut attrs = BTreeMap::new();
        if let Some(br) = st.brightness {
            attrs.insert("brightness".into(), (br.0 as u64).into());
        }
        match st.color {
            Some(hub_core::cap::light::LightColor::Temperature { mireds }) => {
                attrs.insert("mireds".into(), (mireds.0 as u64).into());
            }
            Some(hub_core::cap::light::LightColor::Rgb { rgb }) => {
                attrs.insert("rgb".into(), serde_json::json!([rgb.r, rgb.g, rgb.b]));
            }
            None => {}
        }
        StateUpdate {
            entity_id,
            value: serde_json::Value::Bool(matches!(st.power, hub_core::cap::light::Power::On)),
            attributes: attrs,
            ts: Utc::now(),
            source: Some("adapter-sdk:light".into()),
        }
    }
}
