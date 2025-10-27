use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::model::{Entity, EntityDomain, EntityId};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BinarySensorDeviceClass {
    Door,
    Window,
    Motion,
    Occupancy,
    Moisture,
    Smoke,
    Vibration,
    Generic,
}

impl BinarySensorDeviceClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Door => "door",
            Self::Window => "window",
            Self::Motion => "motion",
            Self::Occupancy => "occupancy",
            Self::Moisture => "moisture",
            Self::Smoke => "smoke",
            Self::Vibration => "vibration",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BinarySensorDescription {
    pub entity_id: EntityId,
    pub device_class: Option<BinarySensorDeviceClass>,
    pub inverted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BinarySensorState {
    pub on: bool,
}

impl TryFrom<&Entity> for BinarySensorDescription {
    type Error = &'static str;

    fn try_from(entity: &Entity) -> Result<Self, Self::Error> {
        if entity.domain != EntityDomain::Sensor && entity.domain != EntityDomain::BinarySensor {
            return Err("not a binary sensor entity");
        }

        let device_class =
            entity.attributes.get("device_class").and_then(|v| v.as_str()).and_then(|s| match s {
                "door" => Some(BinarySensorDeviceClass::Door),
                "window" => Some(BinarySensorDeviceClass::Window),
                "motion" => Some(BinarySensorDeviceClass::Motion),
                "occupancy" => Some(BinarySensorDeviceClass::Occupancy),
                "moisture" => Some(BinarySensorDeviceClass::Moisture),
                "smoke" => Some(BinarySensorDeviceClass::Smoke),
                "vibration" => Some(BinarySensorDeviceClass::Vibration),
                "generic" => Some(BinarySensorDeviceClass::Generic),
                _ => None,
            });

        let inverted = entity.attributes.get("inverted").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(Self { entity_id: entity.id, device_class, inverted })
    }
}

impl BinarySensorState {
    pub fn from_entity_state(value: &Value, attrs: &BTreeMap<String, Value>) -> Option<Self> {
        let mut on = match value {
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("on") || s == "open" => true,
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("off") || s == "closed" => false,
            _ => false,
        };

        if let Some(b) = attrs.get("on").and_then(|v| v.as_bool()) {
            on = b;
        }

        Some(Self { on })
    }
}
