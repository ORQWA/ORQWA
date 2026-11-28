//! The corridor around a route: which search cells to ask a source about, and where an image
//! lies relative to the road.
//!
//! A source is asked by bounding box, so the corridor is cut into a fixed grid: every cell the
//! corridor touches is searched once, and images are then kept by their true distance to the
//! route rather than by which cell they fell in.

use std::collections::BTreeSet;

use orqwa_domain::imagery::{LatLon, StreetImage};

/// Edge of a search cell in degrees: about 550 m north–south, less east–west away from the
/// equator. Small enough that a city's cell stays under a source's result limit, large enough
/// that a course needs a few dozen cells rather than thousands.
pub const CELL_DEGREES: f64 = 0.005;

/// Metres per degree of latitude (WGS84 mean).
const METRES_PER_DEGREE: f64 = 111_132.0;

/// How far apart the corridor is sampled when collecting cells, in metres. Smaller than a cell,
/// so no cell the corridor merely crosses is missed.
const CELL_STEP: f64 = 10.0;

/// A cell of the search grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cell {
    /// Column, from the antimeridian east.
    pub x: i32,
    /// Row, from the south pole north.
    pub y: i32,
}

impl Cell {
    /// The cell holding `point`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // grid indices of a position on Earth
    pub fn of((lat, lon): LatLon) -> Self {
        Self {
            x: (lon / CELL_DEGREES).floor() as i32,
            y: (lat / CELL_DEGREES).floor() as i32,
        }
    }

    /// Its bounding box as (min longitude, min latitude, max longitude, max latitude).
    #[must_use]
    pub fn bounds(self) -> (f64, f64, f64, f64) {
        let west = f64::from(self.x) * CELL_DEGREES;
        let south = f64::from(self.y) * CELL_DEGREES;
        (west, south, west + CELL_DEGREES, south + CELL_DEGREES)
    }

    /// The name it is cached under.
    #[must_use]
    pub fn key(self) -> String {
        format!("{}_{}", self.x, self.y)
    }
}

/// An image placed against the route.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    /// The image.
    pub image: StreetImage,
    /// Distance from the start of the route, in metres.
    pub along: f64,
    /// Distance from the route, in metres.
    pub offset: f64,
    /// Which side of the road it lies on, travelling from the first point.
    pub side: Side,
}

/// The side of the road an image lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Left of the direction of travel.
    Left,
    /// Right of the direction of travel.
    Right,
    /// On the line itself.
    On,
}

/// The cells the corridor around `points` touches, in a stable order.
#[must_use]
pub fn cells(points: &[LatLon], width: f64) -> Vec<Cell> {
    let mut touched = BTreeSet::new();
    for point in resample(points, CELL_STEP) {
        let cell = Cell::of(point);
        for dx in -1..=1 {
            for dy in -1..=1 {
                let candidate = Cell {
                    x: cell.x + dx,
                    y: cell.y + dy,
                };
                if rect_distance(candidate, point) <= width {
                    touched.insert(candidate);
                }
            }
        }
    }
    touched.into_iter().collect()
}

/// How far `at` lies from the polyline `points`: (distance, distance along the line), both in
/// metres, or `None` for an empty line.
#[must_use]
pub fn distance_to_route(points: &[LatLon], at: LatLon) -> Option<(f64, f64)> {
    nearest(points, at).map(|(distance, along, _)| (distance, along))
}

/// Places `images` against the route, nearest first.
#[must_use]
pub fn locate(points: &[LatLon], images: &[StreetImage]) -> Vec<Located> {
    let mut located: Vec<Located> = images
        .iter()
        .filter_map(|image| {
            let (offset, along, cross) = nearest(points, image.position)?;
            Some(Located {
                image: image.clone(),
                along,
                offset,
                side: if cross.abs() < 0.01 {
                    Side::On
                } else if cross > 0.0 {
                    Side::Left
                } else {
                    Side::Right
                },
            })
        })
        .collect();
    located.sort_by(|a, b| a.along.total_cmp(&b.along));
    located
}

/// How far `at` lies from the line, how far along it, and which side (positive is left).
fn nearest(points: &[LatLon], at: LatLon) -> Option<(f64, f64, f64)> {
    let first = *points.first()?;
    let (lat0, lon0) = first;
    let east = METRES_PER_DEGREE * lat0.to_radians().cos();
    let to_xy = |(lat, lon): LatLon| ((lon - lon0) * east, (lat - lat0) * METRES_PER_DEGREE);
    let (px, py) = to_xy(at);

    if points.len() == 1 {
        let (ax, ay) = to_xy(first);
        return Some(((px - ax).hypot(py - ay), 0.0, 0.0));
    }

    let mut best: Option<(f64, f64, f64)> = None;
    let mut along = 0.0;
    for pair in points.windows(2) {
        let (ax, ay) = to_xy(pair[0]);
        let (bx, by) = to_xy(pair[1]);
        let (dx, dy) = (bx - ax, by - ay);
        let length = dx.hypot(dy);
        let t = if length > 0.0 {
            (((px - ax) * dx + (py - ay) * dy) / (length * length)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (ax + t * dx, ay + t * dy);
        let distance = (px - cx).hypot(py - cy);
        let cross = dx * (py - ay) - dy * (px - ax);
        if best.is_none_or(|(closest, _, _)| distance < closest) {
            best = Some((distance, along + t * length, cross));
        }
        along += length;
    }
    best
}

/// How far `point` lies from a cell's rectangle, in metres.
fn rect_distance(cell: Cell, point: LatLon) -> f64 {
    let (west, south, east, north) = cell.bounds();
    let (lat, lon) = point;
    let metres_east = METRES_PER_DEGREE * lat.to_radians().cos();
    let dx = (west - lon).max(0.0).max(lon - east) * metres_east;
    let dy = (south - lat).max(0.0).max(lat - north) * METRES_PER_DEGREE;
    dx.hypot(dy)
}

/// The line, resampled at most `step` metres apart.
fn resample(points: &[LatLon], step: f64) -> Vec<LatLon> {
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let mut out = vec![first];
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let metres = ((b.0 - a.0) * METRES_PER_DEGREE)
            .hypot((b.1 - a.1) * METRES_PER_DEGREE * a.0.to_radians().cos());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (metres / step).ceil() as usize;
        for n in 1..=steps {
            #[allow(clippy::cast_precision_loss)]
            let t = n as f64 / steps as f64;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, lat: f64, lon: f64) -> StreetImage {
        StreetImage {
            id: id.to_owned(),
            position: (lat, lon),
            heading: 0.0,
            captured_at: 0,
        }
    }

    #[test]
    fn a_point_beside_a_north_south_road_is_measured_from_it() {
        let route = [(46.0, 7.0), (46.01, 7.0)];
        // 0.0001° of longitude at 46° N is about 7.7 m.
        let (distance, along) = distance_to_route(&route, (46.005, 7.0001)).unwrap();
        assert!((distance - 7.73).abs() < 0.1, "{distance}");
        assert!((along - 556.0).abs() < 2.0, "{along}");
    }

    #[test]
    fn a_point_past_the_end_is_measured_from_the_end() {
        let route = [(46.0, 7.0), (46.01, 7.0)];
        let (distance, along) = distance_to_route(&route, (46.02, 7.0)).unwrap();
        assert!((distance - 1111.0).abs() < 2.0, "{distance}");
        assert!((along - 1111.0).abs() < 2.0, "{along}");
    }

    #[test]
    fn an_empty_line_has_no_distance() {
        assert!(distance_to_route(&[], (46.0, 7.0)).is_none());
    }

    #[test]
    fn the_corridor_includes_the_cells_it_only_touches() {
        // A point just west of a cell edge must bring in the cell on the other side.
        let edge = (7.0 / CELL_DEGREES).floor() * CELL_DEGREES;
        let route = [(46.0, edge - 0.000_01)];
        let found = cells(&route, 30.0);
        let own = Cell::of(route[0]);
        assert!(found.contains(&own));
        assert!(found.contains(&Cell {
            x: own.x + 1,
            y: own.y
        }));
    }

    #[test]
    fn a_short_route_needs_only_its_own_cell() {
        // Well inside a cell, away from its edges.
        let route = [(46.0025, 7.0025), (46.003, 7.003)];
        let found = cells(&route, 30.0);
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn images_are_placed_on_the_side_they_lie_on() {
        let route = [(46.0, 7.0), (46.01, 7.0)];
        let images = [image("east", 46.005, 7.0001), image("west", 46.005, 6.9999)];
        let located = locate(&route, &images);
        assert_eq!(located[0].image.id, "east");
        assert_eq!(located[0].side, Side::Right);
        assert_eq!(located[1].side, Side::Left);
        assert!(located[0].offset < 10.0);
    }

    #[test]
    fn located_images_come_in_route_order() {
        let route = [(46.0, 7.0), (46.01, 7.0)];
        let images = [image("later", 46.008, 7.0), image("earlier", 46.002, 7.0)];
        let located = locate(&route, &images);
        assert_eq!(located[0].image.id, "earlier");
        assert_eq!(located[1].image.id, "later");
    }
}
