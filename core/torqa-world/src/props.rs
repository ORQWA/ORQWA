//! What stands by the road ridden (R92): guard rails where the ground falls away beside it,
//! delineator posts along country roads and snow poles on mountain roads; in towns street
//! lamps, benches with bins and bus shelters; in the countryside pasture fences, hay bales and
//! wayside crosses. Placed by rules from
//! the road's shape, the ground beside it and the land around, as the Blender models of
//! `art/props` (`app/assets/models/props`).

use std::collections::BTreeMap;

use torqa_osm::LandCover;
use torqa_routes::Surface;

use crate::landcover::LandIndex;
use crate::road::{Mouth, RoadIndex, Station};
use crate::streets::Clearance;
use crate::vegetation::Footprint;
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
/// Street lamps in towns: this far apart on each side, staggered (one side's halfway between
/// the other's), at the road's edge.
const LAMP_SPACING: f64 = 60.0;
const LAMP_OFFSET: f64 = ROAD_HALF_WIDTH + 0.7;
/// Benches face the road from this far out, at about one place in `BENCH_SHARE` of every
/// `TOWN_STEP`; a bin stands beside each, `BIN_ALONG` along the road.
const TOWN_STEP: f64 = 10.0;
const BENCH_OFFSET: f64 = ROAD_HALF_WIDTH + 2.2;
const BENCH_SHARE: f64 = 0.035;
const BIN_ALONG: f64 = 1.4;
/// A bus stop's shelter every `SHELTER_SPACING` of road in town, on the right of travel, its
/// front this far out.
const SHELTER_SPACING: f64 = 400.0;
const SHELTER_OFFSET: f64 = ROAD_HALF_WIDTH + 2.4;
/// What stands in town keeps this clear of buildings and side streets.
const TOWN_CLEAR: f64 = 1.0;
/// Where the map has no built-up land around a village, its buildings make it one: this many
/// within this far of the road's point.
const VILLAGE_BUILDINGS: usize = 4;
const VILLAGE_REACH: f64 = 60.0;
/// Pasture fences: spans this long, this far out, along stretches of meadow or field; a
/// stretch is `FENCE_STRETCH` long and about one in `FENCE_SHARE` is fenced on each side.
const FENCE_SPAN: f64 = 3.0;
const FENCE_OFFSET: f64 = ROAD_HALF_WIDTH + 3.0;
const FENCE_STRETCH: f64 = 120.0;
const FENCE_SHARE: f64 = 0.3;
/// Hay bales lie in fields between these distances from the road, at about one spot in
/// `BALE_SHARE` of a `BALE_GRID` grid, one to three together.
const BALE_GRID: f64 = 18.0;
const BALE_NEAR: f64 = 12.0;
const BALE_FAR: f64 = 90.0;
const BALE_SHARE: f64 = 0.06;
/// A wayside cross stands beside about one in `CROSS_SHARE` of the country lanes joining the
/// road, just past the lane, this far out.
const CROSS_SHARE: f64 = 0.2;
const CROSS_OFFSET: f64 = ROAD_HALF_WIDTH + 2.5;
/// The map's own way of the road ridden may lie this far off the drawn road: its points there
/// are the road, not a side street.
const ROAD_WAY_SLACK: f64 = 4.0;

/// Things standing by the road in one chunk.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Props {
    /// Instances by model name, as Godot `MultiMesh` buffers of 12 floats each: the transform
    /// relative to the chunk centre (row-major 3×4).
    pub models: BTreeMap<String, Vec<f32>>,
}

/// What props are placed by: the chunk's ground, the land cover, the road ridden, where streets
/// join it, the streets' room and the buildings around.
#[derive(Clone, Copy)]
pub(crate) struct Roadside<'a> {
    pub(crate) heights: &'a HeightGrid,
    pub(crate) land: &'a LandIndex,
    pub(crate) road: &'a RoadIndex,
    pub(crate) mouths: &'a [Mouth],
    pub(crate) streets: &'a Clearance,
    pub(crate) buildings: &'a [Footprint],
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
    town(&mut props, low, high, roadside, chunk_origin);
    fences(&mut props, low, high, roadside, chunk_origin);
    crosses(&mut props, low, high, roadside, chunk_origin);
    bales(&mut props, low, high, roadside, chunk_origin);
    props
}

/// Whether the land at a point is pasture or field: mapped so, or open land the map leaves
/// unmarked, which in the country is mostly grazed or mown.
fn pasture(roadside: &Roadside, east: f64, north: f64) -> bool {
    matches!(
        roadside.land.cover_at(east, north),
        Some(LandCover::Meadow | LandCover::Farmland) | None
    )
}

/// Pasture fences span by span along stretches where the land beside the road is meadow or
/// field, outside towns and where no rail stands; whether a stretch is fenced is decided once
/// for it, so fences run on rather than flicker.
fn fences(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    for station in roadside.road.stations(FENCE_SPAN, low, high) {
        if station.surface != Surface::Ground || built_up(&station, roadside) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)] // stretch numbers along the route
        let stretch = (station.distance / FENCE_STRETCH).floor() as i64;
        for side in [-1.0, 1.0] {
            #[allow(clippy::cast_possible_truncation)] // ±1
            let salt = side as i64;
            let fenced = crate::hash(stretch.wrapping_mul(31) ^ salt ^ 0x6665_6e63) < FENCE_SHARE;
            let (east, north) = out(&station, side, FENCE_OFFSET);
            if !fenced
                || !pasture(roadside, east, north)
                || falls_away(&station, side, roadside)
                || room(&station, side, FENCE_OFFSET, 0.2, roadside).is_none()
            {
                continue;
            }
            let at = beside(&station, side, FENCE_OFFSET, roadside);
            push(
                props,
                "fence",
                at,
                &station,
                side,
                station.grade,
                chunk_origin,
            );
        }
    }
}

/// A wayside cross just past about one in `CROSS_SHARE` of the lanes joining the road outside
/// towns, facing the road.
fn crosses(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    for station in roadside.road.stations(TOWN_STEP, low, high) {
        if station.surface != Surface::Ground || built_up(&station, roadside) {
            continue;
        }
        for &(along, side, half) in roadside.mouths {
            let past = station.distance - (along + half + 3.0);
            #[allow(clippy::cast_possible_truncation)] // whole metres along the route
            let seed = along.round() as i64;
            if !(0.0..TOWN_STEP).contains(&past) || crate::hash(seed ^ 0x6372_6f73) >= CROSS_SHARE {
                continue;
            }
            if let Some(at) = room(&station, side, CROSS_OFFSET, 0.4, roadside) {
                push(
                    props,
                    "wayside_cross",
                    at,
                    &station,
                    side,
                    0.0,
                    chunk_origin,
                );
            }
        }
    }
}

/// Round hay bales, straw or wrapped, lying in fields near the road: one to three together at
/// a few spots of a grid, each turned its own way.
#[allow(clippy::cast_possible_truncation)] // grid cells and small counts
fn bales(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    let first = (
        (low.0 / BALE_GRID).ceil() as i64,
        (low.1 / BALE_GRID).ceil() as i64,
    );
    let last = (
        (high.0 / BALE_GRID).ceil() as i64,
        (high.1 / BALE_GRID).ceil() as i64,
    );
    for i in first.0..last.0 {
        for j in first.1..last.1 {
            let seed = i.wrapping_mul(92_821) ^ j.wrapping_mul(68_917) ^ 0x6261_6c65;
            if crate::hash(seed) >= BALE_SHARE {
                continue;
            }
            #[allow(clippy::cast_precision_loss)] // grid cells
            let (east, north) = (i as f64 * BALE_GRID, j as f64 * BALE_GRID);
            let near = roadside
                .road
                .nearest(east, north, BALE_FAR)
                .map(|(d, _, _)| d);
            if !near.is_some_and(|d| d > BALE_NEAR)
                || roadside.land.cover_at(east, north) != Some(LandCover::Farmland)
            {
                continue;
            }
            #[allow(clippy::cast_sign_loss)] // hash is in [0, 1)
            let count = 1 + (crate::hash(seed ^ 1) * 3.0) as usize;
            let model = if crate::hash(seed ^ 2) < 0.4 {
                "hay_bale_wrapped"
            } else {
                "hay_bale"
            };
            for k in 0..count {
                #[allow(clippy::cast_possible_wrap)] // small counts
                let salt = seed ^ (k as i64 + 3).wrapping_mul(7919);
                let (e, n) = (
                    east + (crate::hash(salt) - 0.5) * 6.0,
                    north + (crate::hash(salt ^ 5) - 0.5) * 6.0,
                );
                let blocked = roadside.streets.blocked(e, n, 1.5)
                    || roadside
                        .buildings
                        .iter()
                        .any(|b| b.wall_distance(e, n) < 2.0);
                if blocked {
                    continue;
                }
                let yaw = crate::hash(salt ^ 9) * std::f64::consts::TAU;
                let ground = roadside.heights.at(e, n);
                let lying = Station {
                    position: (e, n),
                    elevation: ground,
                    distance: 0.0,
                    direction: (yaw.cos(), yaw.sin()),
                    grade: 0.0,
                    surface: Surface::Ground,
                };
                push(props, model, [e, ground, n], &lying, 1.0, 0.0, chunk_origin);
            }
        }
    }
}

/// In towns: street lamps staggered along both sides, now and then a bench facing the road with
/// a bin beside it, and a bus shelter every `SHELTER_SPACING`; each only where it has room
/// (clear of buildings, side streets and where streets join).
fn town(
    props: &mut Props,
    low: (f64, f64),
    high: (f64, f64),
    roadside: &Roadside,
    chunk_origin: [f64; 3],
) {
    for station in roadside.road.stations(TOWN_STEP, low, high) {
        if station.surface != Surface::Ground || !built_up(&station, roadside) {
            continue;
        }
        let step = (station.distance / TOWN_STEP).round();
        // Lamps: on the right at whole multiples of the spacing, on the left half way between.
        let half = LAMP_SPACING / TOWN_STEP / 2.0;
        if step % half == 0.0 {
            let side = if (step / half) % 2.0 == 0.0 {
                1.0
            } else {
                -1.0
            };
            if let Some(at) = room(&station, side, LAMP_OFFSET, 0.4, roadside) {
                push(props, "street_lamp", at, &station, side, 0.0, chunk_origin);
            }
            continue;
        }
        #[allow(clippy::cast_possible_truncation)] // whole metres along the route
        let seed = station.distance.round() as i64;
        if (station.distance / SHELTER_SPACING).fract().abs() < 1e-6 {
            if let Some(at) = room(&station, 1.0, SHELTER_OFFSET, 2.5, roadside) {
                push(props, "bus_shelter", at, &station, 1.0, 0.0, chunk_origin);
            }
            continue;
        }
        if crate::hash(seed ^ 0x6265_6e63) < BENCH_SHARE {
            let side = if crate::hash(seed ^ 0x7369_6465) < 0.5 {
                1.0
            } else {
                -1.0
            };
            if let Some(at) = room(&station, side, BENCH_OFFSET, 1.2, roadside) {
                push(props, "bench", at, &station, side, 0.0, chunk_origin);
                let bin = Station {
                    position: (
                        station.position.0 + station.direction.0 * BIN_ALONG,
                        station.position.1 + station.direction.1 * BIN_ALONG,
                    ),
                    ..station
                };
                if let Some(at) = room(&bin, side, BENCH_OFFSET, 0.3, roadside) {
                    push(props, "bin", at, &bin, side, 0.0, chunk_origin);
                }
            }
        }
    }
}

/// The spot `offset` metres out on `side`, on the ground, if something of `reach` metres
/// fits there: off the road's edge where no street joins, clear of streets and buildings.
fn room(
    station: &Station,
    side: f64,
    offset: f64,
    reach: f64,
    roadside: &Roadside,
) -> Option<[f64; 3]> {
    if !open(station, side, roadside) {
        return None;
    }
    let (east, north) = out(station, side, offset);
    // The road ridden is one of the map's streets too: only the others count.
    let other_street = |e: f64, n: f64| {
        roadside
            .road
            .nearest(e, n, ROAD_HALF_WIDTH + ROAD_WAY_SLACK)
            .is_none()
    };
    let blocked = roadside
        .streets
        .blocked_by(east, north, reach + TOWN_CLEAR, other_street)
        || roadside
            .buildings
            .iter()
            .any(|b| b.wall_distance(east, north) < reach + TOWN_CLEAR);
    (!blocked).then(|| beside(station, side, offset, roadside))
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

/// Whether the land at the road is built up: towns get lamps, not posts. Built-up land on the
/// map, or where it maps none, a few buildings close to the road.
fn built_up(station: &Station, roadside: &Roadside) -> bool {
    let (east, north) = station.position;
    let mapped = matches!(
        roadside.land.cover_at(east, north),
        Some(
            LandCover::Residential
                | LandCover::Industrial
                | LandCover::Commercial
                | LandCover::Public
        )
    );
    mapped
        || roadside
            .buildings
            .iter()
            .filter(|b| b.distance(east, north) < VILLAGE_REACH)
            .count()
            >= VILLAGE_BUILDINGS
}

/// Whether the road's edge on `side` is free here: no street joins it nearby.
fn open(station: &Station, side: f64, roadside: &Roadside) -> bool {
    !roadside.mouths.iter().any(|&(along, mouth_side, half)| {
        // Sides are ±1.
        (mouth_side - side).abs() < 1.0 && (along - station.distance).abs() < half + MOUTH_CLEAR
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
    /// Every model the rules place, as `art/props` builds them.
    const PLACED: [&str; 13] = [
        "guard_rail",
        "guard_rail_end",
        "guard_rail_start",
        "delineator",
        "snow_pole",
        "street_lamp",
        "bench",
        "bin",
        "bus_shelter",
        "fence",
        "hay_bale",
        "hay_bale_wrapped",
        "wayside_cross",
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
            "metal_dark",
            "wood",
            "lamp",
            "glass",
            "wood_weathered",
            "hay",
            "wrap",
            "stone",
        ] {
            crate::palette::srgb(&format!("props.{material}"), 1.0);
        }
    }
}
