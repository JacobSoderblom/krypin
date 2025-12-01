use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    bus_contract::CommandSet,
    model::{Entity, EntityDomain, EntityId},
};

bitflags::bitflags! {
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct HvacFeatures: u32 {
        const ONOFF            = 0b0001;
        const TARGET_TEMPERATURE = 0b0010;
        const FAN_MODES        = 0b0100;
        const MODES            = 0b1000;
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Temperature(pub f32);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HvacMode {
    Off,
    Heat,
    Cool,
    Auto,
    Dry,
    FanOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HvacFanMode {
    Auto,
    Low,
    Medium,
    High,
    Turbo,
    Quiet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HvacDescription {
    pub entity_id: EntityId,
    pub features: HvacFeatures,
    pub min_temp: Option<Temperature>,
    pub max_temp: Option<Temperature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HvacState {
    pub mode: HvacMode,
    pub target_temperature: Option<Temperature>,
    pub ambient_temperature: Option<Temperature>,
    pub fan_mode: Option<HvacFanMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HvacCommand {
    SetMode { mode: HvacMode },
    SetTargetTemperature { temperature: Temperature },
    SetFanMode { fan_mode: HvacFanMode },
}

impl HvacDescription {
    pub fn validate(&self, cmd: &HvacCommand) -> Result<(), &'static str> {
        match cmd {
            HvacCommand::SetMode { mode } => {
                if self.features.contains(HvacFeatures::MODES) {
                    if matches!(mode, HvacMode::Off) && !self.features.contains(HvacFeatures::ONOFF) {
                        Err("on/off unsupported")
                    } else {
                        Ok(())
                    }
                } else {
                    Err("modes unsupported")
                }
            }
            HvacCommand::SetTargetTemperature { temperature } => {
                if !self.features.contains(HvacFeatures::TARGET_TEMPERATURE) {
                    return Err("temperature unsupported");
                }
                if let Some(min) = self.min_temp {
                    if temperature.0 < min.0 {
                        return Err("temperature below minimum");
                    }
                }
                if let Some(max) = self.max_temp {
                    if temperature.0 > max.0 {
                        return Err("temperature above maximum");
                    }
                }
                Ok(())
            }
            HvacCommand::SetFanMode { .. } => {
                if self.features.contains(HvacFeatures::FAN_MODES) {
                    Ok(())
                } else {
                    Err("fan modes unsupported")
                }
            }
        }
    }
}

impl TryFrom<&Entity> for HvacDescription {
    type Error = &'static str;

    fn try_from(entity: &Entity) -> Result<Self, Self::Error> {
        if entity.domain != EntityDomain::Climate {
            return Err("not a climate entity");
        }

        let mut features = HvacFeatures::ONOFF | HvacFeatures::MODES;

        if let Some(bits) = entity.attributes.get("features").and_then(|v| v.as_u64()) {
            features = HvacFeatures::from_bits_truncate(bits as u32);
        } else {
            if entity.attributes.get("fan_modes").and_then(|v| v.as_bool()) == Some(true) {
                features |= HvacFeatures::FAN_MODES;
            }
            if entity.attributes.get("target_temperature").and_then(|v| v.as_bool()) == Some(true) {
                features |= HvacFeatures::TARGET_TEMPERATURE;
            }
        }

        let min_temp = entity
            .attributes
            .get("min_temp_c")
            .and_then(|v| v.as_f64())
            .map(|v| Temperature(v as f32));
        let max_temp = entity
            .attributes
            .get("max_temp_c")
            .and_then(|v| v.as_f64())
            .map(|v| Temperature(v as f32));

        Ok(Self { entity_id: entity.id, features, min_temp, max_temp })
    }
}

impl HvacState {
    pub fn from_entity_state(
        value: &serde_json::Value,
        attrs: &BTreeMap<String, serde_json::Value>,
    ) -> Option<Self> {
        let mode = match value.as_str() {
            Some("heat") => HvacMode::Heat,
            Some("cool") => HvacMode::Cool,
            Some("auto") => HvacMode::Auto,
            Some("dry") => HvacMode::Dry,
            Some("fan_only") => HvacMode::FanOnly,
            _ => HvacMode::Off,
        };

        let target_temperature = attrs
            .get("target_temperature_c")
            .and_then(|v| v.as_f64())
            .map(|v| Temperature(v as f32));
        let ambient_temperature = attrs
            .get("ambient_temperature_c")
            .and_then(|v| v.as_f64())
            .map(|v| Temperature(v as f32));
        let fan_mode = attrs
            .get("fan_mode")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "auto" => Some(HvacFanMode::Auto),
                "low" => Some(HvacFanMode::Low),
                "medium" => Some(HvacFanMode::Medium),
                "high" => Some(HvacFanMode::High),
                "turbo" => Some(HvacFanMode::Turbo),
                "quiet" => Some(HvacFanMode::Quiet),
                _ => None,
            });

        Some(HvacState { mode, target_temperature, ambient_temperature, fan_mode })
    }
}

impl From<HvacCommand> for CommandSet {
    fn from(cmd: HvacCommand) -> Self {
        match cmd {
            HvacCommand::SetMode { mode } => Self {
                action: "set_mode".into(),
                value: serde_json::json!({ "mode": mode }),
                correlation_id: None,
            },
            HvacCommand::SetTargetTemperature { temperature } => Self {
                action: "set_temperature".into(),
                value: serde_json::json!({ "target_temperature_c": temperature.0 }),
                correlation_id: None,
            },
            HvacCommand::SetFanMode { fan_mode } => Self {
                action: "set_fan_mode".into(),
                value: serde_json::json!({ "fan_mode": fan_mode }),
                correlation_id: None,
            },
        }
    }
}
