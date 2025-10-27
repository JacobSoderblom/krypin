use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::bus_contract::CommandSet;
use crate::model::{Entity, EntityDomain, EntityId};

bitflags::bitflags! {
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LightFeatures: u32 {
        const ONOFF      = 0b0001;
        const DIMMABLE   = 0b0010;
        const COLOR_TEMP = 0b0100;
        const RGB        = 0b1000;
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Brightness(pub u8); // 0..=100 normalized

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Mireds(pub u16); // typical 153-500

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightColor {
    Temperature { mireds: Mireds },
    Rgb { rgb: Rgb },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LightDescription {
    pub entity_id: EntityId,
    pub features: LightFeatures,
    pub min_mireds: Option<Mireds>,
    pub max_mireds: Option<Mireds>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Power {
    Off,
    On,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LightState {
    pub power: Power,
    pub brightness: Option<Brightness>,
    pub color: Option<LightColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightCommand {
    SetPower { on: bool },
    Toggle,
    SetBrightness { level: Brightness, transition_ms: Option<u32> },
    SetColorTemp { mireds: Mireds, transition_ms: Option<u32> },
    SetRgb { rgb: Rgb, transition_ms: Option<u32> },
}

impl LightDescription {
    pub fn validate(&self, cmd: &LightCommand) -> Result<(), &'static str> {
        match cmd {
            LightCommand::SetPower { .. } | LightCommand::Toggle => {
                if self.features.contains(LightFeatures::ONOFF) {
                    Ok(())
                } else {
                    Err("on/off unsupported")
                }
            }
            LightCommand::SetBrightness { .. } => {
                if self.features.contains(LightFeatures::DIMMABLE) {
                    Ok(())
                } else {
                    Err("dimming unsupported")
                }
            }
            LightCommand::SetColorTemp { .. } => {
                if self.features.contains(LightFeatures::COLOR_TEMP) {
                    Ok(())
                } else {
                    Err("color temp unsupported")
                }
            }
            LightCommand::SetRgb { .. } => {
                if self.features.contains(LightFeatures::RGB) {
                    Ok(())
                } else {
                    Err("rgb unsupported")
                }
            }
        }
    }
}

impl TryFrom<&Entity> for LightDescription {
    type Error = &'static str;

    fn try_from(e: &Entity) -> Result<Self, Self::Error> {
        if e.domain != EntityDomain::Light {
            return Err("not a light entity");
        }

        let mut features = LightFeatures::ONOFF;
        if let Some(v) = e.attributes.get("features").and_then(|v| v.as_u64()) {
            features = LightFeatures::from_bits_truncate(v as u32);
        } else {
            if e.attributes.get("dimmable").and_then(|v| v.as_bool()) == Some(true) {
                features |= LightFeatures::DIMMABLE;
            }
            if e.attributes.get("color_temp").and_then(|v| v.as_bool()) == Some(true) {
                features |= LightFeatures::COLOR_TEMP;
            }
            if e.attributes.get("rgb").and_then(|v| v.as_bool()) == Some(true) {
                features |= LightFeatures::RGB;
            }
        }

        let min_mireds =
            e.attributes.get("min_mireds").and_then(|v| v.as_u64()).map(|n| Mireds(n as u16));
        let max_mireds =
            e.attributes.get("max_mireds").and_then(|v| v.as_u64()).map(|n| Mireds(n as u16));

        Ok(Self { entity_id: e.id, features, min_mireds, max_mireds })
    }
}

impl LightState {
    pub fn from_entity_state(
        value: &serde_json::Value,
        attrs: &BTreeMap<String, serde_json::Value>,
    ) -> Option<Self> {
        let power = match value {
            serde_json::Value::Bool(b) => {
                if *b {
                    Power::On
                } else {
                    Power::Off
                }
            }
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("on") => Power::On,
            serde_json::Value::String(s) if s.eq_ignore_ascii_case("off") => Power::Off,
            _ => Power::Off,
        };

        let brightness = if let Some(v) = attrs.get("brightness") {
            if let Some(n) = v.as_u64() {
                let n = n as u32;
                // Heuristic: if > 100, treat as 0..255
                let pct = if n > 100 { ((n.min(255) * 100) / 255) as u8 } else { n as u8 };
                Some(Brightness(pct))
            } else {
                None
            }
        } else {
            None
        };

        let color = if let Some(m) = attrs.get("mireds").and_then(|v| v.as_u64()) {
            Some(LightColor::Temperature { mireds: Mireds(m as u16) })
        } else if let (Some(r), Some(g), Some(b)) = (
            attrs.get("r").and_then(|v| v.as_u64()),
            attrs.get("g").and_then(|v| v.as_u64()),
            attrs.get("b").and_then(|v| v.as_u64()),
        ) {
            Some(LightColor::Rgb { rgb: Rgb { r: r as u8, g: g as u8, b: b as u8 } })
        } else if let Some(rgb_arr) = attrs.get("rgb").and_then(|v| v.as_array()) {
            if rgb_arr.len() == 3 {
                let r = rgb_arr[0].as_u64().unwrap_or(0) as u8;
                let g = rgb_arr[1].as_u64().unwrap_or(0) as u8;
                let b = rgb_arr[2].as_u64().unwrap_or(0) as u8;
                Some(LightColor::Rgb { rgb: Rgb { r, g, b } })
            } else {
                None
            }
        } else {
            None
        };

        Some(LightState { power, brightness, color })
    }
}

impl From<LightCommand> for CommandSet {
    fn from(cmd: LightCommand) -> Self {
        match cmd {
            LightCommand::SetPower { on } => Self {
                action: "set".into(),
                value: serde_json::json!({ "on": on }),
                correlation_id: None,
            },
            LightCommand::Toggle => Self {
                action: "toggle".into(),
                value: serde_json::Value::Null,
                correlation_id: None,
            },
            LightCommand::SetBrightness { level, transition_ms } => Self {
                action: "set".into(),
                value: serde_json::json!({
                    "brightness": level.0,      // normalized 0..100
                    "transition_ms": transition_ms
                }),
                correlation_id: None,
            },
            LightCommand::SetColorTemp { mireds, transition_ms } => Self {
                action: "set".into(),
                value: serde_json::json!({
                    "mireds": mireds.0,
                    "transition_ms": transition_ms
                }),
                correlation_id: None,
            },
            LightCommand::SetRgb { rgb, transition_ms } => Self {
                action: "set".into(),
                value: serde_json::json!({
                    "rgb": [rgb.r, rgb.g, rgb.b],
                    "transition_ms": transition_ms
                }),
                correlation_id: None,
            },
        }
    }
}
