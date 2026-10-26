//! How far the land around the route lies from it, on a coarse grid, for the ground's shader
//! (R146): woods the generator thins out, beyond [`DENSE_FOREST`] from the route, are drawn as
//! a closed canopy wherever the camera is, a free camera above them included. Worked out from
//! the route when a world is drawn, so kept worlds need nothing new.

use std::collections::HashMap;

use crate::CORRIDOR;
use crate::vegetation::NEAR_DISTANCE;

/// Within this distance of the route woods keep all their trees (R103); beyond, the
/// generator thins them out.
pub const DENSE_FOREST: f64 = NEAR_DISTANCE;
/// Distances are kept in steps of this many metres, up to 255 steps.
pub const DISTANCE_STEP: f64 = 2.0;
/// Cells are at least this large, and as large as it takes to keep the grid within
/// [`MAX_CELLS`] a side.
const MIN_CELL: f64 = 16.0;
const MAX_CELLS: f64 = 2048.0;
/// Segments are found through buckets this large: at least the largest distance kept, so the
/// nearest segment within it lies in a cell's own bucket or a neighbouring one.
const BUCKET: f64 = 512.0;

/// Distances from the route over the land around it.
#[derive(Debug, Clone, PartialEq)]
pub struct RouteDistances {
    /// East and north (metres from the route start) of the grid's north-west corner.
    pub north_west: [f64; 2],
    /// Edge length of a cell in metres.
    pub cell: f64,
    /// Cells from west to east.
    pub columns: usize,
    /// Cells from north to south.
    pub rows: usize,
    /// Row by row from the north, each from the west: the distance of the cell's centre from
    /// the route in [`DISTANCE_STEP`]s, 255 for that far or farther.
    pub steps: Vec<u8>,
}

impl RouteDistances {
    /// The distance in metres the grid keeps for the cell holding (`east`, `north`); the
    /// largest it keeps outside the grid.
    #[must_use]
    pub fn at(&self, east: f64, north: f64) -> f64 {
        let column = ((east - self.north_west[0]) / self.cell).floor();
        let row = ((self.north_west[1] - north) / self.cell).floor();
        #[allow(clippy::cast_precision_loss)] // grids are at most 2048 cells a side
        let inside =
            column >= 0.0 && row >= 0.0 && column < self.columns as f64 && row < self.rows as f64;
        if !inside {
            return f64::from(u8::MAX) * DISTANCE_STEP;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked above
        let index = row as usize * self.columns + column as usize;
        f64::from(self.steps[index]) * DISTANCE_STEP
    }
}

/// The distances from the route through `points` (metres east and north of its start) over
/// the corridor around it.
#[must_use]
pub fn route_distances(points: &[(f64, f64)]) -> RouteDistances {
    let Some(&(first_east, first_north)) = points.first() else {
        return RouteDistances {
            north_west: [0.0, 0.0],
            cell: MIN_CELL,
            columns: 0,
            rows: 0,
            steps: Vec::new(),
        };
    };
    let (mut west, mut east, mut south, mut north) =
        (first_east, first_east, first_north, first_north);
    for &(e, n) in points {
        west = west.min(e);
        east = east.max(e);
        south = south.min(n);
        north = north.max(n);
    }
    let reach = CORRIDOR + MIN_CELL;
    let (west, east, south, north) = (west - reach, east + reach, south - reach, north + reach);
    let cell = MIN_CELL.max((east - west).max(north - south) / MAX_CELLS);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // positive, at most 2048
    let (columns, rows) = (
        ((east - west) / cell).ceil() as usize,
        ((north - south) / cell).ceil() as usize,
    );
    let segments: Vec<[(f64, f64); 2]> = if points.len() == 1 {
        vec![[points[0], points[0]]]
    } else {
        points.windows(2).map(|pair| [pair[0], pair[1]]).collect()
    };
    let buckets = buckets(&segments);
    let farthest = f64::from(u8::MAX) * DISTANCE_STEP;
    let mut steps = Vec::with_capacity(columns * rows);
    for row in 0..rows {
        #[allow(clippy::cast_precision_loss)] // at most 2048
        let n = north - (row as f64 + 0.5) * cell;
        for column in 0..columns {
            #[allow(clippy::cast_precision_loss)] // at most 2048
            let e = west + (column as f64 + 0.5) * cell;
            let distance = nearby(&buckets, (e, n))
                .map(|segment| segment_distance((e, n), segments[segment]))
                .fold(farthest, f64::min);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // 0 to 255
            steps.push((distance / DISTANCE_STEP).round().min(f64::from(u8::MAX)) as u8);
        }
    }
    RouteDistances {
        north_west: [west, north],
        cell,
        columns,
        rows,
        steps,
    }
}

type Bucket = (i64, i64);

#[allow(clippy::cast_possible_truncation)] // world coordinates are far below 2^63 buckets
fn bucket((e, n): (f64, f64)) -> Bucket {
    ((e / BUCKET).floor() as i64, (n / BUCKET).floor() as i64)
}

/// Each segment under every bucket its bounds touch.
fn buckets(segments: &[[(f64, f64); 2]]) -> HashMap<Bucket, Vec<usize>> {
    let mut buckets: HashMap<Bucket, Vec<usize>> = HashMap::new();
    for (index, &[a, b]) in segments.iter().enumerate() {
        let low = bucket((a.0.min(b.0), a.1.min(b.1)));
        let high = bucket((a.0.max(b.0), a.1.max(b.1)));
        for x in low.0..=high.0 {
            for y in low.1..=high.1 {
                buckets.entry((x, y)).or_default().push(index);
            }
        }
    }
    buckets
}

/// The segments in the bucket of `point` and its neighbours: all within `BUCKET` of it.
fn nearby(
    buckets: &HashMap<Bucket, Vec<usize>>,
    point: (f64, f64),
) -> impl Iterator<Item = usize> + '_ {
    let (x, y) = bucket(point);
    (x - 1..=x + 1)
        .flat_map(move |bx| (y - 1..=y + 1).map(move |by| (bx, by)))
        .filter_map(|key| buckets.get(&key))
        .flatten()
        .copied()
}

fn segment_distance(point: (f64, f64), [start, end]: [(f64, f64); 2]) -> f64 {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length2 = dx * dx + dy * dy;
    let along = if length2 > 0.0 {
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point.0 - (start.0 + along * dx)).hypot(point.1 - (start.1 + along * dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_are_measured_from_the_route_not_its_points() {
        // A straight route 2 km east: the land 120 m north of its middle is 120 m away, though
        // its nearest point is a kilometre off.
        let field = route_distances(&[(0.0, 0.0), (2000.0, 0.0)]);
        assert!((field.at(1000.0, 120.0) - 120.0).abs() <= field.cell);
        assert!((field.at(1000.0, -450.0) - 450.0).abs() <= field.cell);
        assert!(field.at(1000.0, 0.0) <= field.cell);
    }

    #[test]
    fn woods_beyond_the_dense_belt_read_as_thinned() {
        let field = route_distances(&[(0.0, 0.0), (0.0, 3000.0), (2500.0, 3000.0)]);
        // Inside the bend, near both legs: the nearest decides.
        assert!(field.at(200.0, 2800.0) < DENSE_FOREST);
        assert!(field.at(900.0, 1500.0) > DENSE_FOREST);
        // Far beyond the belt and outside the grid: as far as the grid reaches.
        let farthest = f64::from(u8::MAX) * DISTANCE_STEP;
        assert!(farthest > DENSE_FOREST + 150.0);
        assert!((field.at(0.0, -1450.0) - farthest).abs() < f64::EPSILON);
        assert!((field.at(-50_000.0, 0.0) - farthest).abs() < f64::EPSILON);
    }

    #[test]
    fn the_grid_covers_the_corridor_and_stays_small_for_long_routes() {
        let short = route_distances(&[(0.0, 0.0), (1000.0, 0.0)]);
        assert!((short.cell - MIN_CELL).abs() < f64::EPSILON);
        assert!(short.north_west[0] <= -CORRIDOR && short.north_west[1] >= CORRIDOR);
        assert_eq!(short.steps.len(), short.columns * short.rows);
        let long = route_distances(&[(0.0, 0.0), (120_000.0, 40_000.0)]);
        assert!(long.columns <= 2048 && long.rows <= 2048);
        assert!(long.at(60_000.0, 20_000.0) <= long.cell);
    }

    #[test]
    fn a_route_without_points_has_no_grid() {
        let field = route_distances(&[]);
        assert_eq!(field.columns * field.rows, 0);
        assert!(field.at(0.0, 0.0) >= DENSE_FOREST);
    }
}
