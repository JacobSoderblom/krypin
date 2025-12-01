use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    bus_contract::CommandSet,
    model::{Entity, EntityDomain, EntityId},
};

bitflags::bitflags! {
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct RobotVacFeatures: u32 {
        const START   = 0b0001;
        const PAUSE   = 0b0010;
        const STOP    = 0b0100;
        const DOCK    = 0b1000;
        const LOCATE  = 0b1_0000;
        const SPOT    = 0b10_0000;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RobotVacStatus {
    Idle,
    Cleaning,
    Paused,
    Returning,
    Docked,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RobotVacDescription {
    pub entity_id: EntityId,
    pub features: RobotVacFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RobotVacState {
    pub status: RobotVacStatus,
    pub battery_level: Option<u8>,
    pub fan_power: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RobotVacCommand {
    Start,
    Pause,
    Stop,
    Dock,
    Locate,
    SpotClean,
}

impl RobotVacDescription {
    pub fn validate(&self, cmd: &RobotVacCommand) -> Result<(), &'static str> {
        let requires = match cmd {
            RobotVacCommand::Start => RobotVacFeatures::START,
            RobotVacCommand::Pause => RobotVacFeatures::PAUSE,
            RobotVacCommand::Stop => RobotVacFeatures::STOP,
            RobotVacCommand::Dock => RobotVacFeatures::DOCK,
            RobotVacCommand::Locate => RobotVacFeatures::LOCATE,
            RobotVacCommand::SpotClean => RobotVacFeatures::SPOT,
        };
        if self.features.contains(requires) {
            Ok(())
        } else {
            Err("command unsupported")
        }
    }
}

impl TryFrom<&Entity> for RobotVacDescription {
    type Error = &'static str;

    fn try_from(entity: &Entity) -> Result<Self, Self::Error> {
        if entity.domain != EntityDomain::RobotVacuum {
            return Err("not a robot vacuum entity");
        }

        let mut features = RobotVacFeatures::START | RobotVacFeatures::STOP | RobotVacFeatures::DOCK;

        if let Some(bits) = entity.attributes.get("features").and_then(|v| v.as_u64()) {
            features = RobotVacFeatures::from_bits_truncate(bits as u32);
        } else {
            if entity.attributes.get("pause").and_then(|v| v.as_bool()) == Some(true) {
                features |= RobotVacFeatures::PAUSE;
            }
            if entity.attributes.get("locate").and_then(|v| v.as_bool()) == Some(true) {
                features |= RobotVacFeatures::LOCATE;
            }
            if entity.attributes.get("spot_clean").and_then(|v| v.as_bool()) == Some(true) {
                features |= RobotVacFeatures::SPOT;
            }
        }

        Ok(Self { entity_id: entity.id, features })
    }
}

impl RobotVacState {
    pub fn from_entity_state(
        value: &serde_json::Value,
        attrs: &BTreeMap<String, serde_json::Value>,
    ) -> Option<Self> {
        let status = match value.as_str() {
            Some("cleaning") => RobotVacStatus::Cleaning,
            Some("paused") => RobotVacStatus::Paused,
            Some("returning") => RobotVacStatus::Returning,
            Some("docked") => RobotVacStatus::Docked,
            Some("error") => RobotVacStatus::Error,
            _ => RobotVacStatus::Idle,
        };

        let battery_level = attrs.get("battery").and_then(|v| v.as_u64()).map(|v| v as u8);
        let fan_power = attrs.get("fan_power").and_then(|v| v.as_u64()).map(|v| v as u8);

        Some(RobotVacState { status, battery_level, fan_power })
    }
}

impl From<RobotVacCommand> for CommandSet {
    fn from(cmd: RobotVacCommand) -> Self {
        match cmd {
            RobotVacCommand::Start => Self { action: "start".into(), value: serde_json::Value::Null, correlation_id: None },
            RobotVacCommand::Pause => Self { action: "pause".into(), value: serde_json::Value::Null, correlation_id: None },
            RobotVacCommand::Stop => Self { action: "stop".into(), value: serde_json::Value::Null, correlation_id: None },
            RobotVacCommand::Dock => Self { action: "dock".into(), value: serde_json::Value::Null, correlation_id: None },
            RobotVacCommand::Locate => Self { action: "locate".into(), value: serde_json::Value::Null, correlation_id: None },
            RobotVacCommand::SpotClean => Self { action: "spot_clean".into(), value: serde_json::Value::Null, correlation_id: None },
        }
    }
}
