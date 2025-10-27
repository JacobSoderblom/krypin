use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    bus_contract::CommandSet,
    model::{Entity, EntityDomain, EntityId},
};

bitflags::bitflags! {
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SwitchFeatures: u32 {
        const ONOFF       = 0b0001;
        const TOGGLE      = 0b0010;
        const STATELESS   = 0b0100;
        const POWER_METER = 0b1000;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwitchDescription {
    pub entity_id: EntityId,
    pub features: SwitchFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwitchState {
    pub on: bool,
    pub power_w: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SwitchCommand {
    Set { on: bool },
    Toggle,
}

impl SwitchDescription {
    pub fn validate(&self, cmd: &SwitchCommand) -> Result<(), &'static str> {
        match cmd {
            SwitchCommand::Set { .. } => {
                if self.features.contains(SwitchFeatures::ONOFF) {
                    Ok(())
                } else {
                    Err("on/off unsupported")
                }
            }
            SwitchCommand::Toggle => {
                if self.features.contains(SwitchFeatures::TOGGLE) {
                    Ok(())
                } else {
                    Err("toggle unsupported")
                }
            }
        }
    }
}

impl TryFrom<&Entity> for SwitchDescription {
    type Error = &'static str;

    fn try_from(entity: &Entity) -> Result<Self, Self::Error> {
        if entity.domain != EntityDomain::Switch {
            return Err("not a switch entity");
        }
        let mut features = SwitchFeatures::ONOFF;

        if let Some(bits) = entity.attributes.get("features").and_then(|v| v.as_u64()) {
            features = SwitchFeatures::from_bits_truncate(bits as u32);
        } else {
            if entity.attributes.get("toggle").and_then(|v| v.as_bool()) == Some(true) {
                features |= SwitchFeatures::TOGGLE;
            }
            if entity.attributes.get("stateless").and_then(|v| v.as_bool()) == Some(true) {
                features |= SwitchFeatures::STATELESS;
            }
            if entity.attributes.get("power_meter").and_then(|v| v.as_bool()) == Some(true) {
                features |= SwitchFeatures::POWER_METER;
            }
        }

        Ok(Self { entity_id: entity.id, features })
    }
}

impl SwitchState {
    pub fn from_entity_state(
        value: &serde_json::Value,
        attrs: &BTreeMap<String, serde_json::Value>,
    ) -> Option<Self> {
        let on = match value {
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("on") => true,
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("off") => false,
            _ => false,
        };
        let power_w = attrs.get("power_w").and_then(|v| v.as_f64()).map(|w| w as f32);
        Some(Self { on, power_w })
    }
}

impl From<SwitchCommand> for CommandSet {
    fn from(cmd: SwitchCommand) -> Self {
        match cmd {
            SwitchCommand::Set { on } => Self {
                action: "set".into(),
                value: serde_json::json!({ "on": on }),
                correlation_id: None,
            },
            SwitchCommand::Toggle => Self {
                action: "toggle".into(),
                value: serde_json::Value::Null,
                correlation_id: None,
            },
        }
    }
}
