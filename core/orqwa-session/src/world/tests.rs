//! A world's script in a ride (R302, R303): what it asks is carried out within the limits, and
//! a script that stops leaves a plain ride.

use std::fmt::Write as _;
use std::time::Duration;

use orqwa_domain::telemetry::{Telemetry, TrainerControl};
use orqwa_domain::units::{Meters, Percent, Rpm, Watts};
use orqwa_routes::Route;
use orqwa_world_script::WorldScript;
use serde_json::{Value, json};

use super::{Request, WorldDriver, WorldRider};
use crate::{Ride, RideConfig};

/// A module answering every tick with `output`.
fn answering(output: &Value) -> WorldScript {
    let text = output.to_string();
    let escaped = text.bytes().fold(String::new(), |mut out, b| {
        let _ = write!(out, "\\{b:02x}");
        out
    });
    let wat = format!(
        r#"(module (memory (export "memory") 2)
             (data (i32.const 0) "{escaped}")
             (func (export "alloc") (param i32) (result i32) (i32.const 32768))
             (func (export "tick") (param i32 i32) (result i64)
               (i64.shl (i64.const {}) (i64.const 32))))"#,
        text.len()
    );
    WorldScript::load(wat.as_bytes(), 1).unwrap()
}

/// A straight, flat route due north, `length` metres long.
async fn flat(length: f64) -> Route {
    let mut xml = String::from("<gpx><trk><trkseg>");
    let degrees_per_meter = 180.0 / (std::f64::consts::PI * 6_371_000.0);
    for i in 0..=10 {
        let lat = 46.0 + f64::from(i) * length / 10.0 * degrees_per_meter;
        let _ = write!(xml, r#"<trkpt lat="{lat}" lon="7"><ele>500</ele></trkpt>"#);
    }
    xml.push_str("</trkseg></trk></gpx>");
    Route::from_gpx(&xml, None).await.unwrap()
}

fn rider() -> WorldRider {
    WorldRider {
        name: "Ana".into(),
        ftp: Watts(250.0),
    }
}

async fn ride_with(output: &Value) -> Ride {
    let driver = WorldDriver::new(answering(output), Value::Null, "loop", rider());
    Ride::new(flat(5000.0).await, RideConfig::default()).with_world_script(driver)
}

/// Pedals `seconds` at `power`, returning the controls sent.
fn pedal(ride: &mut Ride, power: f64, seconds: u32) -> Vec<TrainerControl> {
    ride.on_telemetry(&Telemetry {
        power: Some(Watts(power)),
        cadence: Some(Rpm(90.0)),
        ..Telemetry::default()
    });
    (0..seconds * 10)
        .filter_map(|_| ride.tick(Duration::from_millis(100)))
        .collect()
}

#[tokio::test]
async fn a_script_asks_the_trainer_for_power_within_twice_the_ftp() {
    let output = json!({"trainer": [{"slot": 0, "kind": "power", "power_w": 900}]});
    let mut ride = ride_with(&output).await;
    let controls = pedal(&mut ride, 200.0, 3);
    assert_eq!(
        controls.last(),
        Some(&TrainerControl::TargetPower(Watts(500.0)))
    );
}

#[tokio::test]
async fn a_scripts_grade_is_within_real_roads_and_scaled_by_the_difficulty() {
    let output = json!({"trainer": [{"slot": 0, "kind": "grade", "grade": 0.6}]});
    let mut ride = ride_with(&output).await;
    let controls = pedal(&mut ride, 200.0, 3);
    let Some(TrainerControl::Simulation(simulation)) = controls.last() else {
        panic!("{controls:?}");
    };
    // 25 % at most, at the default 50 % difficulty.
    assert!((simulation.grade.0 - 12.5).abs() < 1e-9, "{simulation:?}");
}

#[tokio::test]
async fn a_scripted_body_moves_the_rider_at_its_speed_whatever_they_pedal() {
    let output = json!({"bodies": [
        {"id": "pace", "kind": "scripted", "riders": [0], "speed_mps": 5.0}
    ]});
    let mut ride = ride_with(&output).await;
    pedal(&mut ride, 400.0, 10);
    let distance = ride.state().distance.0;
    assert!((distance - 50.0).abs() < 1.0, "{distance}");
    // A held start: speed 0 stays put.
    let output = json!({"bodies": [{"id": "start", "kind": "scripted", "riders": [0]}]});
    let mut held = ride_with(&output).await;
    pedal(&mut held, 400.0, 10);
    assert!(held.state().distance.0 < 1.0);
}

#[tokio::test]
async fn a_heavier_body_is_slower_and_its_factors_are_limited() {
    let mut light = ride_with(&json!({})).await;
    let output = json!({"bodies": [
        {"id": "load", "kind": "driven", "riders": [0], "mass_kg": 5000, "rolling": 40}
    ]});
    let mut heavy = ride_with(&output).await;
    pedal(&mut light, 200.0, 60);
    pedal(&mut heavy, 200.0, 60);
    assert!(heavy.state().distance.0 < light.state().distance.0 * 0.8);
    let driver = heavy.world().unwrap();
    assert_eq!(
        driver.movement(),
        super::Movement::Driven {
            added: orqwa_domain::units::Kilograms(500.0),
            rolling: 3.0,
            air: 1.0
        }
    );
}

#[tokio::test]
async fn a_structured_workouts_targets_come_before_the_scripts() {
    let workout = crate::workout::Workout::ConstantPower(Watts(180.0));
    let output = json!({"trainer": [{"slot": 0, "kind": "power", "power_w": 300}]});
    let mut ride = ride_with(&output).await.with_workout(workout);
    let controls = pedal(&mut ride, 200.0, 3);
    assert_eq!(
        controls.last(),
        Some(&TrainerControl::TargetPower(Watts(180.0)))
    );
}

#[tokio::test]
async fn results_and_marks_are_kept_and_unknown_requests_are_the_road() {
    let mut ride = ride_with(&json!({
        "trainer": [{"slot": 0, "kind": "cadence"}],
        "record": {"values": {"score": 3}, "marks": [{"name": "lap"}]}
    }))
    .await;
    pedal(&mut ride, 200.0, 1);
    let driver = ride.world().unwrap();
    assert_eq!(driver.request(), Request::Road);
    assert_eq!(driver.values().get("score"), Some(&3.0));
    assert_eq!(driver.marks().len(), 10, "one per tick that gave it");
}

#[tokio::test]
async fn a_script_that_stops_leaves_a_plain_ride() {
    let trap = r#"(module (memory (export "memory") 1)
        (func (export "alloc") (param i32) (result i32) (i32.const 0))
        (func (export "tick") (param i32 i32) (result i64) (unreachable)))"#;
    let driver = WorldDriver::new(
        WorldScript::load(trap.as_bytes(), 1).unwrap(),
        Value::Null,
        "loop",
        rider(),
    );
    let mut ride = Ride::new(flat(5000.0).await, RideConfig::default()).with_world_script(driver);
    let controls = pedal(&mut ride, 200.0, 10);
    assert!(ride.world().unwrap().stopped().is_some());
    assert!(matches!(
        controls.last(),
        Some(TrainerControl::Simulation(_))
    ));
    assert!(ride.state().distance.0 > Meters(10.0).0);
}

#[test]
fn requests_that_are_not_numbers_are_the_road() {
    let request = |kind: &str, value: f64| {
        let request = serde_json::from_value(json!({"slot": 0, "kind": kind})).map(
            |mut r: orqwa_world_api::TrainerRequest| {
                r.grade = value;
                r.power_w = value;
                r.percent = value;
                r
            },
        );
        Request::limited(&request.unwrap(), Watts(250.0))
    };
    assert_eq!(request("grade", f64::NAN), Request::Road);
    assert_eq!(request("power", f64::INFINITY), Request::Road);
    assert_eq!(
        request("resistance", 140.0),
        Request::Resistance(Percent(100.0))
    );
    assert_eq!(
        Request::limited(
            &serde_json::from_value(json!({"slot": 0, "kind": "power", "power_w": 100})).unwrap(),
            Watts(0.0)
        ),
        Request::Road,
        "no FTP, no power target"
    );
}

#[tokio::test]
async fn a_body_without_the_rider_moves_on_its_own_from_the_start() {
    let output = json!({"bodies": [
        {"id": "pacer", "kind": "scripted", "riders": [], "speed_mps": 4.0}
    ]});
    let mut ride = ride_with(&output).await;
    pedal(&mut ride, 200.0, 10);
    let driver = ride.world().unwrap();
    let pacer = driver.body_distance("pacer", 0.0).unwrap();
    assert!((pacer - 39.6).abs() < 0.5, "{pacer}");
    assert_eq!(driver.body_distance("own-0", 12.0), Some(12.0));
    assert_eq!(driver.body_distance("nobody", 12.0), None);
}

fn mate(slot: u8, distance: f64) -> super::Mate {
    super::Mate {
        slot,
        name: format!("Rider {slot}"),
        power: Watts(250.0),
        cadence: 90.0,
        mass: orqwa_domain::units::Kilograms(80.0),
        ftp: Watts(240.0),
        distance,
        speed: orqwa_domain::units::MetersPerSecond(8.0),
    }
}

#[tokio::test]
async fn a_shared_body_moves_by_everyones_power_and_keeps_its_riders_together() {
    let output = json!({"bodies": [{"id": "team", "kind": "driven", "riders": [0, 1]}]});
    let mut ride = ride_with(&output).await;
    // Alone the body carries the rider only; with a mate further on, it closes on them.
    ride.set_world_team(0, vec![mate(1, 40.0)]);
    pedal(&mut ride, 200.0, 10);
    let driver = ride.world().unwrap();
    let load = driver.load(ride.state().distance.0).unwrap();
    assert_eq!(load.power, Watts(250.0));
    assert!(
        ride.state().distance.0 > 30.0,
        "{:?}",
        ride.state().distance
    );
    assert_eq!(driver.slot_distance(1, 0.0), Some(40.0));
    assert_eq!(driver.slot_distance(0, 7.0), Some(7.0));
}

#[tokio::test]
async fn in_a_group_the_script_answers_for_this_riders_slot() {
    let output = json!({"trainer": [
        {"slot": 0, "kind": "power", "power_w": 400},
        {"slot": 2, "kind": "power", "power_w": 180}
    ]});
    let mut ride = ride_with(&output).await;
    ride.set_world_team(2, vec![mate(0, 0.0)]);
    pedal(&mut ride, 200.0, 2);
    assert_eq!(
        ride.world().unwrap().request(),
        Request::Power(Watts(180.0))
    );
}
