use std::collections::BTreeMap;

use hub_core::{
    bus_contract::CommandSet,
    cap::light::{
        Brightness, LightColor, LightCommand, LightDescription, LightFeatures, LightState, Mireds,
        Power, Rgb,
    },
    model::{DeviceId, Entity, EntityDomain, EntityId},
};
use serde_json::json;
use uuid::Uuid;

fn mk_entity_with_attrs(attrs: BTreeMap<String, serde_json::Value>) -> Entity {
    Entity {
        id: EntityId(Uuid::new_v4()),
        device_id: DeviceId(Uuid::new_v4()),
        name: "Light".into(),
        domain: EntityDomain::Light,
        icon: None,
        key: Some("sim:1".into()),
        attributes: attrs,
    }
}

#[test]
fn bitflags_serde_roundtrip_readable() {
    let f = LightFeatures::ONOFF | LightFeatures::DIMMABLE | LightFeatures::COLOR_TEMP;
    let s = serde_json::to_string(&f).unwrap();
    let back: LightFeatures = serde_json::from_str(&s).unwrap();
    assert_eq!(back, f);
}

#[test]
fn bitflags_bits_numeric_mask() {
    let f = LightFeatures::ONOFF | LightFeatures::DIMMABLE | LightFeatures::COLOR_TEMP;
    assert_eq!(f.bits(), 0b0111);
    let json_num = serde_json::to_string(&f.bits()).unwrap();
    assert_eq!(json_num, "7");
}

#[test]
fn description_from_entity_with_explicit_features() {
    let mut attrs = BTreeMap::new();

    attrs.insert("features".into(), json!(3));
    attrs.insert("min_mireds".into(), json!(153));
    attrs.insert("max_mireds".into(), json!(500));
    let e = mk_entity_with_attrs(attrs);

    let d = LightDescription::try_from(&e).expect("light desc");

    assert!(d.features.contains(LightFeatures::ONOFF));
    assert!(d.features.contains(LightFeatures::DIMMABLE));
    assert_eq!(d.min_mireds.unwrap().0, 153);
    assert_eq!(d.max_mireds.unwrap().0, 500);
}

#[test]
fn description_infers_features_from_booleans() {
    let mut attrs = BTreeMap::new();
    attrs.insert("dimmable".into(), json!(true));
    attrs.insert("color_temp".into(), json!(true));
    let e = mk_entity_with_attrs(attrs);

    let d = LightDescription::try_from(&e).expect("light desc");
    assert!(d.features.contains(LightFeatures::DIMMABLE));
    assert!(d.features.contains(LightFeatures::COLOR_TEMP));
    assert!(d.features.contains(LightFeatures::ONOFF));
}

#[test]
fn light_state_from_entity_state_boolean_and_brightness_255_normalizes() {
    let mut attrs = BTreeMap::new();
    attrs.insert("brightness".into(), json!(128u64));
    let st = LightState::from_entity_state(&json!(true), &attrs).expect("parse");
    assert_eq!(st.power, Power::On);
    assert_eq!(st.brightness.unwrap().0, ((128u32 * 100) / 255) as u8);
}

#[test]
fn light_state_parses_color_temp_and_rgb() {
    // color temp path
    let mut attrs1 = BTreeMap::new();
    attrs1.insert("mireds".into(), json!(370));
    let st1 = LightState::from_entity_state(&json!("ON"), &attrs1).unwrap();
    match st1.color.unwrap() {
        LightColor::Temperature { mireds } => assert_eq!(mireds.0, 370),
        _ => panic!("expected temperature color"),
    }

    // rgb via separate r/g/b keys
    let mut attrs2 = BTreeMap::new();
    attrs2.insert("r".into(), json!(12));
    attrs2.insert("g".into(), json!(34));
    attrs2.insert("b".into(), json!(56));
    let st2 = LightState::from_entity_state(&json!("OFF"), &attrs2).unwrap();
    match st2.color.unwrap() {
        LightColor::Rgb { rgb } => assert_eq!((rgb.r, rgb.g, rgb.b), (12, 34, 56)),
        _ => panic!("expected rgb color"),
    }

    // rgb via array
    let mut attrs3 = BTreeMap::new();
    attrs3.insert("rgb".into(), json!([1, 2, 3]));
    let st3 = LightState::from_entity_state(&json!(false), &attrs3).unwrap();
    match st3.color.unwrap() {
        LightColor::Rgb { rgb } => assert_eq!((rgb.r, rgb.g, rgb.b), (1, 2, 3)),
        _ => panic!("expected rgb color"),
    }
}

#[test]
fn command_conversion_to_commandset() {
    let cmd = LightCommand::SetBrightness { level: Brightness(42), transition_ms: Some(250) };
    let cs: CommandSet = cmd.into();
    assert_eq!(cs.action, "set");
    // value should be an object with brightness and transition_ms
    let o = cs.value.as_object().expect("json object");
    assert_eq!(o.get("brightness").and_then(|v| v.as_u64()).unwrap(), 42);
    assert_eq!(o.get("transition_ms").and_then(|v| v.as_u64()).unwrap(), 250);
}

#[test]
fn validation_blocks_unsupported_operations() {
    // Only ON/OFF
    let mut attrs = BTreeMap::new();
    attrs.insert("features".into(), json!(LightFeatures::ONOFF.bits()));
    let e = mk_entity_with_attrs(attrs);
    let d = LightDescription::try_from(&e).unwrap();

    // Power OK
    assert!(d.validate(&LightCommand::SetPower { on: true }).is_ok());
    // Dimming not allowed
    assert!(
        d.validate(&LightCommand::SetBrightness { level: Brightness(10), transition_ms: None })
            .is_err()
    );
    // Color temp not allowed
    assert!(
        d.validate(&LightCommand::SetColorTemp { mireds: Mireds(300), transition_ms: None })
            .is_err()
    );
    // RGB not allowed
    assert!(
        d.validate(&LightCommand::SetRgb { rgb: Rgb { r: 1, g: 2, b: 3 }, transition_ms: None })
            .is_err()
    );
}
