//! API version 1 stays as it is for good (R304): every field of its documents is still read and
//! written as it was, and every feature is still offered. These snapshots only ever grow; a
//! test failing here means a change would break released world scripts.

use std::path::Path;

use orqwa_world_api::{Body, FEATURES, Input, Node, Output, TrainerKind};
use serde_json::Value;

fn snapshot(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/v1")
        .join(name);
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// That everything in `snapshot` is in `now` as it was: objects may have gained fields, nothing
/// else may differ.
fn keeps(snapshot: &Value, now: &Value, at: &str) {
    match (snapshot, now) {
        (Value::Object(before), Value::Object(after)) => {
            for (key, value) in before {
                let field = format!("{at}.{key}");
                let now = after.get(key).unwrap_or_else(|| panic!("{field} is gone"));
                keeps(value, now, &field);
            }
        }
        (Value::Array(before), Value::Array(after)) => {
            assert_eq!(before.len(), after.len(), "{at} changed length");
            for (index, (value, now)) in before.iter().zip(after).enumerate() {
                keeps(value, now, &format!("{at}[{index}]"));
            }
        }
        (Value::Number(before), Value::Number(after)) => {
            assert_eq!(before.as_f64(), after.as_f64(), "{at} changed");
        }
        _ => assert_eq!(snapshot, now, "{at} changed"),
    }
}

#[test]
fn every_input_field_of_version_1_is_still_written() {
    let snapshot = snapshot("input.json");
    let input: Input = serde_json::from_value(snapshot.clone()).unwrap();
    keeps(&snapshot, &serde_json::to_value(input).unwrap(), "input");
}

#[test]
fn every_output_field_of_version_1_is_still_read() {
    let snapshot = snapshot("output.json");
    let output: Output = serde_json::from_value(snapshot.clone()).unwrap();
    keeps(&snapshot, &serde_json::to_value(output).unwrap(), "output");
}

#[test]
fn every_feature_of_version_1_is_still_offered() {
    for feature in snapshot("features.json").as_array().unwrap() {
        assert!(
            FEATURES.contains(&feature.as_str().unwrap()),
            "{feature} is gone"
        );
    }
}

#[test]
fn what_a_script_leaves_out_is_as_without_a_script() {
    let output: Output = serde_json::from_str("{}").unwrap();
    assert_eq!(output, Output::default());
    let body: Body = serde_json::from_str(r#"{"id": "b", "kind": "driven"}"#).unwrap();
    assert_eq!((body.rolling, body.air, body.mass_kg), (1.0, 1.0, 0.0));
    let node: Node = serde_json::from_str(r#"{"id": "n"}"#).unwrap();
    assert!(node.visible);
    assert!((node.scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn fields_and_kinds_a_later_api_adds_are_read_without_error() {
    let output: Output = serde_json::from_str(
        r#"{"bodies": [{"id": "b", "kind": "glider", "wings": 2}],
            "trainer": [{"slot": 0, "kind": "cadence", "rpm": 90}],
            "ui": [{"id": "p", "slot": "overlay", "items": [{"kind": "chart"}]}],
            "sound": {"file": "bell.ogg"}}"#,
    )
    .unwrap();
    let resolved = output.resolved();
    assert!(
        resolved.bodies.is_empty(),
        "riders stay on their own bodies"
    );
    assert_eq!(resolved.trainer[0].kind, TrainerKind::Road);
    assert_eq!(resolved.ui, []);
}
