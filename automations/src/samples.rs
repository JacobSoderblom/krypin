use crate::{Action, Condition, NewAutomation, Trigger};
use hub_core::model::EntityId;
use serde_json::json;
use std::collections::BTreeMap;

/// Turn on a light entity when a motion sensor reports activity.
///
/// The rule watches the provided `motion_sensor` entity for a `true` state
/// transition, then issues a `SetEntityState` for the `light_entity`.
pub fn motion_light(motion_sensor: EntityId, light_entity: EntityId) -> NewAutomation {
    NewAutomation {
        name: "motion -> light on".into(),
        description: Some("Turn on the light whenever motion is detected".into()),
        trigger: Trigger::StateChange { entity_id: motion_sensor, from: None, to: None },
        conditions: vec![Condition::EntityStateEquals {
            entity_id: motion_sensor,
            value: json!(true),
        }],
        actions: vec![Action::SetEntityState {
            entity_id: light_entity,
            value: json!("on"),
            attributes: BTreeMap::new(),
        }],
        enabled: true,
    }
}

/// Schedule a thermostat target temperature using a cron expression.
///
/// The `cron` string is matched against `TriggerEvent::TimeFired` events to
/// drive a `SetEntityState` action with the requested `target_temp_celsius`.
pub fn thermostat_schedule(
    thermostat: EntityId,
    target_temp_celsius: f64,
    cron: impl Into<String>,
) -> NewAutomation {
    NewAutomation {
        name: "thermostat schedule".into(),
        description: Some("Apply a scheduled target temperature".into()),
        trigger: Trigger::Time { cron: cron.into() },
        conditions: vec![Condition::Always],
        actions: vec![Action::SetEntityState {
            entity_id: thermostat,
            value: json!(target_temp_celsius),
            attributes: BTreeMap::from([(String::from("unit"), json!("C"))]),
        }],
        enabled: true,
    }
}
