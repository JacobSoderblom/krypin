use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::model::{DeviceId, EntityDomain, EntityId};

pub const TOPIC_DEVICE_ANNOUNCE: &str = "krypin.device.announce";
pub const TOPIC_ENTITY_ANNOUNCE: &str = "krypin.entity.announce";
pub const TOPIC_STATE_UPDATE_PREFIX: &str = "krypin.state.update.";
pub const TOPIC_COMMAND_PREFIX: &str = "krypin.command.";
pub const TOPIC_HEARTBEAT: &str = "krypin.hub.heartbeat";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceAnnounce {
    pub id: DeviceId,
    pub name: String,
    pub adapter: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub sw_version: Option<String>,
    pub hw_version: Option<String>,
    pub area: Option<Uuid>, // optional AreaId by UUID to avoid circular dep in contracts
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityAnnounce {
    pub id: EntityId,
    pub device_id: DeviceId,
    pub name: String,
    pub domain: EntityDomain,
    pub icon: Option<String>,
    pub key: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateUpdate {
    pub entity_id: EntityId,
    pub value: serde_json::Value,
    #[serde(default)]
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub ts: DateTime<Utc>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Heartbeat {
    pub ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandSet {
    pub action: String,           // "set", "toggle", etc.
    pub value: serde_json::Value, // target value
    pub correlation_id: Option<Uuid>,
}
