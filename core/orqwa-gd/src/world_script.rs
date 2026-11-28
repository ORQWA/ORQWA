//! A world script's answer for the scene and the screen (R302): the answer as JSON, read again
//! only when it changes, and where each node and the camera hint stand now, worked out here so
//! the scene only places them.
use godot::prelude::*;
use orqwa_app::App;
use orqwa_routes::{LocalProjection, Route};
use orqwa_world_api::Anchor;

/// `{answers, stopped, output, poses, camera, models}`: `answers` counts the script's answers,
/// `output` is the last one as JSON (`docs/world-files.md`), `poses` one `{x, y, elevation_m,
/// heading}` (or `null`, not placed) per `output.scene.nodes` entry, `camera` the hint's anchor
/// pose with `back_m`, `up_m` and `aim_up_m` (or `null`), `models` the script's model files by
/// name; `stopped` says why the script stopped, empty while it runs. `null` without a script.
pub(super) fn state(app: &App, local: f64) -> Variant {
    let Some(driver) = app.world_script() else {
        return Variant::nil();
    };
    let output = driver.output();
    let route = app.route();
    let pose = |anchor: &Anchor| -> Option<VarDictionary> {
        let distance = match (&anchor.body, anchor.slot, anchor.route_m) {
            (Some(body), None, None) => driver.body_distance(body, local),
            // In a ride on one's own, the only rider is slot 0.
            (None, Some(0), None) => Some(local),
            (None, None, Some(at)) if at.is_finite() => Some(at),
            _ => None,
        };
        Some(place(app, route?, distance?))
    };
    let mut poses = VarArray::new();
    for node in &output.scene.nodes {
        poses.push(
            &pose(&node.anchor)
                .as_ref()
                .map_or_else(Variant::nil, ToGodot::to_variant),
        );
    }
    let camera = output.scene.camera.as_ref().and_then(|camera| {
        let mut at = pose(&camera.anchor)?;
        at.set("back_m", camera.back_m);
        at.set("up_m", camera.up_m);
        at.set("aim_up_m", camera.aim_up_m);
        Some(at)
    });
    let camera = camera
        .as_ref()
        .map_or_else(Variant::nil, ToGodot::to_variant);
    let mut models = VarDictionary::new();
    for (name, file) in app.world_script_models() {
        models.set(name.as_str(), file.as_str());
    }
    let json = serde_json::to_string(output).unwrap_or_default();
    vdict! {
        "answers" => i64::try_from(driver.answers()).unwrap_or(i64::MAX),
        "stopped" => driver.stopped().map(ToString::to_string).unwrap_or_default().as_str(),
        "output" => json.as_str(),
        "poses" => &poses,
        "camera" => &camera,
        "models" => &models,
    }
    .to_variant()
}

/// Where a place `distance` along `route` is in the scene, in the rider's lane.
fn place(app: &App, route: &Route, distance: f64) -> VarDictionary {
    let distance = orqwa_domain::units::Meters(distance.clamp(0.0, route.length().0));
    let at = route.position(distance);
    let (x, y) = LocalProjection::for_route(route).project(at.lat, at.lon);
    let (x, y) = app.rider_position(distance, false).unwrap_or((x, y));
    vdict! {
        "x" => x,
        "y" => y,
        "elevation_m" => at.elevation.0,
        "heading" => at.heading,
    }
}
