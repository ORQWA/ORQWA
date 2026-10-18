//! What stands by the road ridden (R92): guard rails where the ground falls away beside it,
//! delineator posts along country roads and snow poles on mountain roads, placed by rules from
//! the road's shape, the ground beside it and the land around, as the Blender models of
//! `art/props` (`app/assets/models/props`).

use std::collections::BTreeMap;

use torqa_osm::LandCover;
use torqa_routes::Surface;

use crate::landcover::LandIndex;
use crate::road::{Mouth, RoadIndex, Station};
use crate::{HeightGrid, ROAD_HALF_WIDTH};

/// A guard rail's span (`guard_rail`, one post at its middle) and how far its face stands from
/// the road's centre line.
const RAIL_SPAN: f64 = 4.0;
const RAIL_OFFSET: f64 = ROAD_HALF_WIDTH + 0.6;
/// The ground this far beyond the road's edge, this much below the road, calls for a rail.
const DROP_PROBE: f64 = ROAD_HALF_WIDTH + 4.5;
const RAIL_DROP: f64 = 2.0;
/// Delineator posts stand this far apart and this far from the centre line, on both sides.
const POST_SPACING: f64 = 50.0;
const POST_OFFSET: f64 = ROAD_HALF_WIDTH + 0.8;
/// Above this height (metres) snow poles mark the road's edges instead, closer together and a
/// little further out.
const SNOW_LINE: f64 = 1300.0;
const POLE_SPACING: f64 = 25.0;
const POLE_OFFSET: f64 = ROAD_HALF_WIDTH + 1.2;
/// Nothing stands this far either side of where a street joins the road, beyond its half width.
const MOUTH_CLEAR: f64 = 6.0;

/// Things standing by the road in one chunk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Props {
    /// Instances by model name, as Godot `MultiMesh` buffers of 12 floats each: the transform
    /// relative to the chunk centre (row-major 3×4).
    pub models: BTreeMap<String, Vec<f32>>,
}

/// What props are placed by: the chunk's ground, the land cover, the road ridden and where
/// streets join it.
#[derive(Clone, Copy)]
pub(crate) struct Roadside<'a> {
    pub(crate) heights: &'a HeightGrid,
    pub(crate) land: &'a LandIndex,
    pub(crate) road: &'a RoadIndex,
    pub(crate) mouths: &'a [Mouth],
}

/// Places the props whose road point lies in the square `[origin, origin + size]` (metres
/// east/north). Each decision depends only on the road and the ground near its own point, so a
/// prop is the same whichever chunk asks.
pub(crate) fn place(
    origin: (f64, f64),
    size: f64,
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) -> Props {
    let mut props = Props::default();
    let (low, high) = (origin, (origin.0 + size, origin.1 + size));
    rails(&mut props, low, high, roadside, chunk_origin);
    posts(&mut props, low, high, roadside, chunk_origin);
    props
}

/// Guard rails on the side where the ground falls away, span by span. A span stands where the
/// drop is there and next to it (so single dips get none and single gaps are bridged), and a
/// run starts and ends in a span sloping down into the ground.
fn rails(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    // Two spans beyond the square, for the neighbours each decision looks at.
    let margin = 2.0 * RAIL_SPAN;
    let stations = roadside.road.stations(
        RAIL_SPAN,
        (low.0 - margin, low.1 - margin),
        (high.0 + margin, high.1 + margin),
    );
    for side in [-1.0, 1.0] {
        let drops: Vec<bool> = stations
            .iter()
            .map(|station| falls_away(station, side, roadside))
            .collect();
        // Neighbours are the stations a span apart; the road may leave and re-enter the area.
        let next_to = |i: usize, step: isize| -> Option<usize> {
            let j = i.checked_add_signed(step)?;
            let apart = stations.get(j)?.distance - stations[i].distance;
            #[allow(clippy::cast_precision_loss)] // one or two steps
            let wanted = RAIL_SPAN * step as f64;
            ((apart - wanted).abs() < 0.5).then_some(j)
        };
        let drop_at = |i: usize, step: isize| next_to(i, step).is_some_and(|j| drops[j]);
        let rail_at = |i: usize| {
            let here = drops[i] && (drop_at(i, -1) || drop_at(i, 1));
            let bridged = drop_at(i, -1) && drop_at(i, 1);
            (here || bridged) && open(&stations[i], side, roadside)
        };
        for (i, station) in stations.iter().enumerate() {
            let (east, north) = station.position;
            let inside = (low.0..high.0).contains(&east) && (low.1..high.1).contains(&north);
            if !inside {
                continue;
            }
            let rail = rail_at(i);
            let before = next_to(i, -1).is_some_and(rail_at);
            let after = next_to(i, 1).is_some_and(rail_at);
            // A run's ends slope down into the ground away from it: `guard_rail_end` down
            // ahead (the model's −z), `guard_rail_start` down behind. Along the road ridden that
            // is ahead on the right and behind on the left.
            let (ahead, behind) = if side > 0.0 {
                ("guard_rail_end", "guard_rail_start")
            } else {
                ("guard_rail_start", "guard_rail_end")
            };
            let model = match (rail, before, after) {
                (true, _, _) => Some("guard_rail"),
                (false, true, false) => Some(ahead),
                (false, false, true) => Some(behind),
                _ => None,
            };
            if let Some(name) = model
                && open(station, side, roadside)
            {
                let at = beside(station, side, RAIL_OFFSET, roadside);
                push(props, name, at, station, side, station.grade, chunk_origin);
            }
        }
    }
}

/// Delineator posts every `POST_SPACING` along country roads, or snow poles every
/// `POLE_SPACING` above the snow line, on both sides; not in towns, on bridges or where a rail
/// stands (its blades carry the reflectors).
fn posts(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    for station in roadside.road.stations(POLE_SPACING, low, high) {
        if station.surface != Surface::Ground || built_up(&station, roadside) {
            continue;
        }
        let snowy = station.elevation >= SNOW_LINE;
        let on_post_spacing = (station.distance / POST_SPACING).fract().abs() < 1e-6;
        if !snowy && !on_post_spacing {
            continue;
        }
        for side in [-1.0, 1.0] {
            if !open(&station, side, roadside) {
                continue;
            }
            let railed = falls_away(&station, side, roadside);
            let (name, offset) = match (snowy, railed) {
                // Behind the rail.
                (true, true) => ("snow_pole", RAIL_OFFSET + 0.5),
                (true, false) => ("snow_pole", POLE_OFFSET),
                (false, false) => ("delineator", POST_OFFSET),
                (false, true) => continue,
            };
            let at = beside(&station, side, offset, roadside);
            push(props, name, at, &station, side, 0.0, chunk_origin);
        }
    }
}

/// Whether the ground falls away beside the road on `side` (1 right of travel, −1 left): open
/// road on the ground, its edge well above the ground a few metres out.
fn falls_away(station: &Station, side: f64, roadside: &Roadside) -> bool {
    if station.surface != Surface::Ground {
        return false;
    }
    let (east, north) = out(station, side, DROP_PROBE);
    station.elevation - roadside.heights.at(east, north) >= RAIL_DROP
}

/// Whether the land at the road is built up: towns get kerbs and lamps, not posts.
fn built_up(station: &Station, roadside: &Roadside) -> bool {
    matches!(
        roadside
            .land
            .cover_at(station.position.0, station.position.1),
        Some(
            LandCover::Residential
                | LandCover::Industrial
                | LandCover::Commercial
                | LandCover::Public
        )
    )
}

/// Whether the road's edge on `side` is free here: no street joins it nearby.
fn open(station: &Station, side: f64, roadside: &Roadside) -> bool {
    !roadside.mouths.iter().any(|&(along, mouth_side, half)| {
        mouth_side == side && (along - station.distance).abs() < half + MOUTH_CLEAR
    })
}

/// The point `offset` metres from the centre line on `side`, on the ground there.
fn beside(station: &Station, side: f64, offset: f64, roadside: &Roadside) -> [f64; 3] {
    let (east, north) = out(station, side, offset);
    [east, roadside.heights.at(east, north), north]
}

/// The point `offset` metres from the centre line on `side`, in metres east/north.
fn out(station: &Station, side: f64, offset: f64) -> (f64, f64) {
    let (de, dn) = station.direction;
    // Right of travel is the direction turned clockwise.
    let (re, rn) = (dn * side, -de * side);
    (
        station.position.0 + re * offset,
        station.position.1 + rn * offset,
    )
}

/// Adds an instance of `name` at `at`, its back to the road on `facing` (its +x away from the
/// road, its length along the road), sheared to follow `grade` so posts stay upright.
#[allow(clippy::cast_possible_truncation)] // geometry is stored as f32 for the GPU
fn push(
    props: &mut Props,
    name: &str,
    [east, height, north]: [f64; 3],
    station: &Station,
    facing: f64,
    grade: f64,
    origin: [f64; 3],
) {
    let (de, dn) = station.direction;
    // Godot axes: x east, y up, z south. The model's +x points away from the road; its −z runs
    // along the road the way it is ridden on the right side and back on the left, rising with
    // the grade.
    let x = [dn * facing, 0.0, de * facing];
    let z = [-de * facing, -grade * facing, dn * facing];
    let (tx, ty, tz) = (east - origin[0], height - origin[1], -north - origin[2]);
    props.models.entry(name.to_owned()).or_default().extend(
        [
            x[0], 0.0, z[0], tx, //
            x[1], 1.0, z[1], ty, //
            x[2], 0.0, z[2], tz,
        ]
        .map(|v| v as f32),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every model the rules place, as `art/props` builds them.
    const PLACED: [&str; 5] = [
        "guard_rail",
        "guard_rail_end",
        "guard_rail_start",
        "delineator",
        "snow_pole",
    ];

    #[test]
    fn every_placed_model_is_built_and_coloured() {
        let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/models/props");
        for name in PLACED {
            let file = format!("{directory}/{name}.glb");
            assert!(std::path::Path::new(&file).exists(), "{file}");
        }
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../app/assets/models/props/models.json"))
                .unwrap();
        for name in PLACED {
            assert!(
                manifest["models"].get(name).is_some(),
                "{name} in models.json"
            );
        }
        // The app colours each material from the palette; `srgb` panics on a missing colour.
        for material in [
            "galvanised",
            "post",
            "band",
            "reflector",
            "pole_red",
            "pole_white",
        ] {
            crate::palette::srgb(&format!("props.{material}"), 1.0);
        }
    }
}
