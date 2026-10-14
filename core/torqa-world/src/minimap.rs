//! The flat maps of the ride's minimap (R19, R70) and of the course cards (#192): map features
//! as coloured triangles in metres east/north of the route start, coloured in the rider's
//! [`MapStyle`] only when drawn. The corridor's map shows the land along the route in detail,
//! for the close view, drawn over the course's map of the whole land ([`crate::course_map`]).

use torqa_domain::profile::MapStyle;
use torqa_osm::{LandCover, MapData, RoadClass};
use torqa_routes::{LocalProjection, Route};

use crate::CORRIDOR;
use crate::buildings::{signed_area, triangulate};
use crate::road::RoadIndex;

/// Rings are simplified to this tolerance in metres; finer detail is invisible on the minimap.
const SIMPLIFY: f64 = 2.5;

/// What a part of a flat map shows, which gives its colour in each [`MapStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapFeature {
    /// Open land: the map's background, and the holes in areas.
    Land,
    /// Towns: residential, industrial and commercial land.
    Built,
    /// Orchards and vineyards.
    Orchard,
    /// Woods.
    Forest,
    /// Bare rock and scree.
    Rock,
    /// Lakes and rivers.
    Water,
    /// Buildings.
    Building,
    /// Streets.
    MinorRoad,
    /// Main roads.
    MajorRoad,
}

impl MapFeature {
    /// Its colour in `style`. The dark style is the course cards' (after CARTO's Dark
    /// Matter); the coloured one is muted to sit under the HUD.
    #[must_use]
    pub fn color(self, style: MapStyle) -> [f32; 4] {
        let rgb = match style {
            MapStyle::Coloured => match self {
                Self::Land => [0.15, 0.17, 0.16],
                Self::Built => [0.20, 0.20, 0.22],
                Self::Orchard => [0.18, 0.22, 0.16],
                Self::Forest => [0.12, 0.20, 0.14],
                Self::Rock => [0.25, 0.25, 0.25],
                Self::Water => [0.11, 0.21, 0.31],
                Self::Building => [0.29, 0.29, 0.32],
                Self::MinorRoad => [0.36, 0.37, 0.40],
                Self::MajorRoad => [0.60, 0.53, 0.37],
            },
            MapStyle::Dark => {
                let grey = match self {
                    Self::Land => 0.055,
                    Self::Orchard | Self::Forest => 0.075,
                    Self::Rock => 0.09,
                    Self::Built => 0.115,
                    Self::Building => 0.14,
                    Self::MinorRoad => 0.15,
                    Self::Water => 0.17,
                    Self::MajorRoad => 0.27,
                };
                [grey; 3]
            }
        };
        [rgb[0], rgb[1], rgb[2], 1.0]
    }
}

/// Triangles, three vertices each, in metres east/north of the route start.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlatMap {
    /// Triangle corners (east, north).
    pub vertices: Vec<[f32; 2]>,
    /// What each vertex shows.
    pub features: Vec<MapFeature>,
}

impl FlatMap {
    /// Each vertex's colour in `style`.
    #[must_use]
    pub fn colors(&self, style: MapStyle) -> Vec<[f32; 4]> {
        self.features.iter().map(|f| f.color(style)).collect()
    }
}

/// The map of the corridor along `route` for the minimap's close view: woods, towns, water,
/// rock, roads and buildings from `map`. Open land (meadows, fields) is left out: the whole
/// land's map below shows through it.
#[must_use]
pub fn minimap(route: &Route, map: &MapData) -> FlatMap {
    let projection = LocalProjection::for_route(route);
    build(map, &projection, &RoadIndex::new(route, &projection))
}

fn build(map: &MapData, projection: &LocalProjection, road: &RoadIndex) -> FlatMap {
    let mut flat = FlatMap::default();
    let near = |points: &[(f64, f64)]| {
        points
            .iter()
            .step_by((points.len() / 16).max(1))
            .any(|&(e, n)| road.nearest(e, n, CORRIDOR).is_some())
    };
    let project = |ring: &[(f64, f64)]| -> Vec<(f64, f64)> {
        ring.iter()
            .map(|&(lat, lon)| projection.project(lat, lon))
            .collect()
    };

    // Land cover from general to specific, so specific areas paint over general ones.
    let mut areas: Vec<_> = map
        .areas
        .iter()
        .filter_map(|a| Some((a, cover_feature(a.cover)?)))
        .collect();
    areas.sort_by_key(|(a, _)| layer(a.cover));
    for (area, feature) in areas {
        for ring in &area.outer {
            let ring = project(ring);
            if near(&ring) {
                fill(&mut flat, &ring, feature, SIMPLIFY);
            }
        }
        // Holes are painted as open land, approximating the cut-out.
        for ring in &area.inner {
            let ring = project(ring);
            if near(&ring) {
                fill(&mut flat, &ring, MapFeature::Land, SIMPLIFY);
            }
        }
    }
    for waterway in &map.waterways {
        ribbon(
            &mut flat,
            &project(&waterway.line),
            waterway.width.max(4.0),
            MapFeature::Water,
            road,
        );
    }
    for minor_first in [false, true] {
        // Paths would clutter the small map.
        let drawn = map.roads.iter().filter(|r| r.class != RoadClass::Path);
        for street in drawn.filter(|r| r.major() == minor_first) {
            let (width, feature) = if street.major() {
                (9.0, MapFeature::MajorRoad)
            } else {
                (5.0, MapFeature::MinorRoad)
            };
            ribbon(&mut flat, &project(&street.line), width, feature, road);
        }
    }
    for building in &map.buildings {
        let outline = project(&building.outline);
        if near(&outline) {
            fill(&mut flat, &outline, MapFeature::Building, SIMPLIFY);
        }
    }
    flat
}

/// Draw order: general land cover first.
fn layer(cover: LandCover) -> u8 {
    match cover {
        LandCover::Meadow | LandCover::Public => 0,
        LandCover::Farmland => 1,
        LandCover::Residential | LandCover::Industrial | LandCover::Commercial => 2,
        LandCover::Orchard => 3,
        LandCover::Forest => 4,
        LandCover::Rock => 5,
        LandCover::Water => 6,
    }
}

/// What the corridor's map shows of `cover`: nothing of open land, which would cover the
/// whole land's map in the corridor's blocks (#236).
fn cover_feature(cover: LandCover) -> Option<MapFeature> {
    match cover {
        // Schools' and hospitals' grounds are mostly lawns and yards.
        LandCover::Meadow | LandCover::Public | LandCover::Farmland => None,
        LandCover::Residential | LandCover::Industrial | LandCover::Commercial => {
            Some(MapFeature::Built)
        }
        LandCover::Orchard => Some(MapFeature::Orchard),
        LandCover::Forest => Some(MapFeature::Forest),
        LandCover::Rock => Some(MapFeature::Rock),
        LandCover::Water => Some(MapFeature::Water),
    }
}

/// Fills a closed ring, simplified to `tolerance` metres.
#[allow(clippy::cast_possible_truncation)] // drawing precision
pub(crate) fn fill(flat: &mut FlatMap, ring: &[(f64, f64)], feature: MapFeature, tolerance: f64) {
    let mut points = simplify(ring, tolerance);
    points.pop(); // closing point
    if points.len() < 3 {
        return;
    }
    if signed_area(&points) < 0.0 {
        points.reverse();
    }
    for triangle in triangulate(&points) {
        for index in triangle {
            let (east, north) = points[index as usize];
            flat.vertices.push([east as f32, north as f32]);
            flat.features.push(feature);
        }
    }
}

/// A band `width` wide along the parts of a line within the corridor.
#[allow(clippy::cast_possible_truncation)] // drawing precision
fn ribbon(
    flat: &mut FlatMap,
    line: &[(f64, f64)],
    width: f64,
    feature: MapFeature,
    road: &RoadIndex,
) {
    let half = width / 2.0;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let middle = (f64::midpoint(a.0, b.0), f64::midpoint(a.1, b.1));
        if road.nearest(middle.0, middle.1, CORRIDOR).is_none() {
            continue;
        }
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        if length < 0.01 {
            continue;
        }
        let (re, rn) = ((b.1 - a.1) / length * half, -(b.0 - a.0) / length * half);
        let corners = [
            (a.0 - re, a.1 - rn),
            (a.0 + re, a.1 + rn),
            (b.0 + re, b.1 + rn),
            (b.0 - re, b.1 - rn),
        ];
        for index in [0, 1, 2, 0, 2, 3] {
            let (east, north) = corners[index];
            flat.vertices.push([east as f32, north as f32]);
            flat.features.push(feature);
        }
    }
}

/// Douglas–Peucker simplification keeping the first and last point.
pub(crate) fn simplify(points: &[(f64, f64)], tolerance: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((first, last)) = stack.pop() {
        let (a, b) = (points[first], points[last]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length = dx.hypot(dy);
        let distance = |p: (f64, f64)| {
            if length < 1e-9 {
                (p.0 - a.0).hypot(p.1 - a.1)
            } else {
                ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / length
            }
        };
        let farthest = (first + 1..last)
            .map(|i| (i, distance(points[i])))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((index, d)) = farthest
            && d > tolerance
        {
            keep[index] = true;
            stack.push((first, index));
            stack.push((index, last));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(&p, k)| k.then_some(p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_sets_water_woods_towns_and_roads_off_the_land() {
        for style in [MapStyle::Coloured, MapStyle::Dark] {
            let land = MapFeature::Land.color(style);
            for feature in [
                MapFeature::Water,
                MapFeature::Forest,
                MapFeature::Built,
                MapFeature::MinorRoad,
                MapFeature::MajorRoad,
            ] {
                assert_ne!(feature.color(style), land, "{style:?} {feature:?}");
            }
            let brightness = |f: MapFeature| f.color(style)[..3].iter().sum::<f32>();
            assert!(brightness(MapFeature::MajorRoad) > brightness(MapFeature::MinorRoad));
        }
        // The dark style is black and greys.
        let [r, g, b, _] = MapFeature::Water.color(MapStyle::Dark);
        assert_eq!([r, g], [g, b]);
    }

    #[test]
    fn simplification_keeps_corners_and_drops_noise() {
        let noisy = [
            (0.0, 0.0),
            (5.0, 0.4),
            (10.0, -0.3),
            (20.0, 0.0),
            (20.0, 20.0),
        ];

        assert_eq!(
            simplify(&noisy, 1.0),
            [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)]
        );
    }
}
