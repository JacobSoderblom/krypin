use std::collections::BTreeMap;

use hub_core::model::{DeviceId, EntityId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::zigbee::ZigbeeInfo;

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
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub zigbee: Option<ZigbeeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMeta {
    pub id: EntityId,
    pub name: String,
    pub icon: Option<String>,
    #[serde(default)]
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
