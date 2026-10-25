//! Giving the tiles' roads what map detail knows of them. The tiles and the detail come from
//! the same OpenStreetMap ways, so a road and its way lie on each other; a way is that road's
//! where it runs along it, of the same kind, within a few metres.

use std::collections::HashMap;

use super::extract::SurfaceWay;
use crate::{LatLon, Road, RoadSurface};

/// Farthest a mapped way may lie from a road to be that road (metres): the tiles keep
/// positions to about half a metre and a parallel sidewalk or lane lies further off.
const NEAR: f64 = 4.0;
/// Least alignment (cosine of the angle) between a road and a mapped way along it.
const ALIGNED: f64 = 0.9;
/// Pieces of road shorter than this (metres) take the surface of the road before them, so a
/// junction leaves no sliver of another surface.
const SHORTEST: f64 = 5.0;
/// Cells of the index of mapped ways (metres).
const CELL: f64 = 25.0;

/// Gives each road the surface of the mapped ways along it, splitting it at its points where
/// that changes. Only a surface the tile's own tells no different of is taken: setts for a
/// paved road, dirt for an unpaved one, either for one the tiles tell nothing of.
pub(crate) fn surfaces(roads: &mut Vec<Road>, ways: &[SurfaceWay]) {
    let Some(origin) = roads.iter().find_map(|road| road.line.first().copied()) else {
        return;
    };
    if ways.is_empty() {
        return;
    }
    let flat = Flat::at(origin);
    let index = Index::new(ways, &flat);
    let mut out = Vec::with_capacity(roads.len());
    for road in roads.drain(..) {
        split(road, &index, &flat, &mut out);
    }
    *roads = out;
}

/// `road` as one road or several, each of one surface, into `out`.
fn split(road: Road, index: &Index, flat: &Flat, out: &mut Vec<Road>) {
    let points: Vec<(f64, f64)> = road.line.iter().map(|&p| flat.xy(p)).collect();
    let mut runs: Vec<Run> = Vec::new();
    for (segment, pair) in points.windows(2).enumerate() {
        let surface = index.surface_along(pair[0], pair[1], &road);
        let length = distance(pair[0], pair[1]);
        match runs.last_mut() {
            Some(run) if run.surface == surface => {
                run.end = segment + 1;
                run.length += length;
            }
            _ => runs.push(Run {
                surface,
                start: segment,
                end: segment + 1,
                length,
            }),
        }
    }
    let runs = without_slivers(runs);
    if let [run] = runs.as_slice() {
        let mut road = road;
        if let Some(surface) = run.surface {
            road.details.surface = surface;
        }
        out.push(road);
        return;
    }
    for run in runs {
        let mut piece = road.clone();
        piece.line = road.line[run.start..=run.end].to_vec();
        if let Some(surface) = run.surface {
            piece.details.surface = surface;
        }
        out.push(piece);
    }
}

/// Consecutive segments (points `start` to `end`) of one surface; `None` where no mapped way
/// tells one.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Run {
    surface: Option<RoadSurface>,
    start: usize,
    end: usize,
    length: f64,
}

/// The runs with each shorter than [`SHORTEST`] joined to the one before it (the first to the
/// one after), and runs of one surface then joined.
fn without_slivers(runs: Vec<Run>) -> Vec<Run> {
    let mut kept: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        match kept.last_mut() {
            Some(last) if run.length < SHORTEST || last.surface == run.surface => {
                last.end = run.end;
                last.length += run.length;
            }
            Some(last) if last.length < SHORTEST => {
                *last = Run {
                    start: last.start,
                    length: last.length + run.length,
                    ..run
                };
            }
            _ => kept.push(run),
        }
    }
    kept
}

/// The mapped ways' segments in cells, projected.
struct Index<'a> {
    ways: &'a [SurfaceWay],
    /// Each way's points, projected.
    lines: Vec<Vec<(f64, f64)>>,
    /// (way, segment) by cell.
    cells: HashMap<(i64, i64), Vec<(usize, usize)>>,
}

impl<'a> Index<'a> {
    fn new(ways: &'a [SurfaceWay], flat: &Flat) -> Self {
        let lines: Vec<Vec<(f64, f64)>> = ways
            .iter()
            .map(|way| {
                way.line
                    .iter()
                    .map(|&[lat, lon]| flat.xy((f64::from(lat) * 1e-7, f64::from(lon) * 1e-7)))
                    .collect()
            })
            .collect();
        let mut cells: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
        for (way, line) in lines.iter().enumerate() {
            for (segment, pair) in line.windows(2).enumerate() {
                let (x0, x1) = (
                    cell(pair[0].0.min(pair[1].0) - NEAR),
                    cell(pair[0].0.max(pair[1].0) + NEAR),
                );
                let (y0, y1) = (
                    cell(pair[0].1.min(pair[1].1) - NEAR),
                    cell(pair[0].1.max(pair[1].1) + NEAR),
                );
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        cells.entry((x, y)).or_default().push((way, segment));
                    }
                }
            }
        }
        Self { ways, lines, cells }
    }

    /// The surface of the road's segment from `a` to `b`: that of the mapped way along most
    /// of it, taken at a quarter, the half and three quarters of its length.
    fn surface_along(&self, a: (f64, f64), b: (f64, f64), road: &Road) -> Option<RoadSurface> {
        let direction = unit(a, b)?;
        let found: Vec<RoadSurface> = [0.25, 0.5, 0.75]
            .iter()
            .filter_map(|&t| {
                let at = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                self.nearest(at, direction, road)
            })
            .collect();
        found
            .iter()
            .copied()
            .find(|&surface| found.iter().filter(|&&s| s == surface).count() >= 2)
    }

    /// The surface of the nearest mapped way of the road's kind along `direction` at `at`.
    fn nearest(&self, at: (f64, f64), direction: (f64, f64), road: &Road) -> Option<RoadSurface> {
        let candidates = self.cells.get(&(cell(at.0), cell(at.1)))?;
        candidates
            .iter()
            .filter(|&&(way, _)| {
                let way = &self.ways[way];
                way.kind == road.details.kind && fits(road.details.surface, way.surface)
            })
            .filter_map(|&(way, segment)| {
                let (p, q) = (self.lines[way][segment], self.lines[way][segment + 1]);
                let along = unit(p, q)?;
                let aligned = (along.0 * direction.0 + along.1 * direction.1).abs();
                let off = distance_to_segment(at, p, q);
                (aligned >= ALIGNED && off <= NEAR).then_some((off, self.ways[way].surface))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, surface)| surface)
    }
}

/// Whether a mapped surface may be that of a road the tiles say `tile` of: they tell paved
/// and unpaved from the same tag.
fn fits(tile: RoadSurface, mapped: RoadSurface) -> bool {
    let paved = matches!(
        mapped,
        RoadSurface::Concrete | RoadSurface::Paving | RoadSurface::Cobbles
    );
    match tile {
        RoadSurface::Unknown => true,
        RoadSurface::Paved => paved,
        RoadSurface::Unpaved => !paved,
        _ => false,
    }
}

/// Positions in metres east and north of an origin; enough for the few kilometres around a
/// road.
struct Flat {
    origin: LatLon,
    east: f64,
}

impl Flat {
    const NORTH: f64 = 111_195.0;

    fn at(origin: LatLon) -> Self {
        Self {
            origin,
            east: Self::NORTH * origin.0.to_radians().cos(),
        }
    }

    fn xy(&self, (lat, lon): LatLon) -> (f64, f64) {
        (
            (lon - self.origin.1) * self.east,
            (lat - self.origin.0) * Self::NORTH,
        )
    }
}

#[allow(clippy::cast_possible_truncation)] // local metres stay far below 2^63 cells
fn cell(v: f64) -> i64 {
    (v / CELL).floor() as i64
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

fn unit(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let length = distance(a, b);
    (length > 1e-6).then(|| ((b.0 - a.0) / length, (b.1 - a.1) / length))
}

fn distance_to_segment(at: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((at.0 - a.0) * dx + (at.1 - a.1) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(at, (a.0 + dx * t, a.1 + dy * t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RoadClass, RoadDetails, RoadKind};

    /// A point `east` and `north` metres from Bern's Zytglogge, as (lat, lon).
    fn at(east: f64, north: f64) -> LatLon {
        let flat = Flat::at((46.948, 7.447));
        (46.948 + north / Flat::NORTH, 7.447 + east / flat.east)
    }

    fn e7((lat, lon): LatLon) -> [i32; 2] {
        #[allow(clippy::cast_possible_truncation)] // positions on the map
        [(lat * 1e7).round() as i32, (lon * 1e7).round() as i32]
    }

    /// A paved local street through these points (metres east, north).
    fn street(points: &[(f64, f64)]) -> Road {
        Road {
            class: RoadClass::Street,
            details: RoadDetails {
                kind: RoadKind::Local,
                surface: RoadSurface::Paved,
                ..RoadDetails::default()
            },
            line: points.iter().map(|&(e, n)| at(e, n)).collect(),
            structure: None,
        }
    }

    fn way(id: i64, kind: RoadKind, surface: RoadSurface, points: &[(f64, f64)]) -> SurfaceWay {
        SurfaceWay {
            id,
            kind,
            surface,
            line: points.iter().map(|&(e, n)| e7(at(e, n))).collect(),
        }
    }

    fn surfaced(roads: Vec<Road>, ways: &[SurfaceWay]) -> Vec<(RoadSurface, usize)> {
        let mut roads = roads;
        surfaces(&mut roads, ways);
        roads
            .iter()
            .map(|road| (road.details.surface, road.line.len()))
            .collect()
    }

    #[test]
    fn a_cobbled_street_is_cobbled_along_its_length() {
        let road = street(&[(0.0, 0.0), (60.0, 0.0), (120.0, 0.0)]);
        // The way as mapped, its points elsewhere along the line and a metre off it.
        let sett = way(
            1,
            RoadKind::Local,
            RoadSurface::Cobbles,
            &[(-5.0, 1.0), (130.0, 1.0)],
        );

        assert_eq!(surfaced(vec![road], &[sett]), [(RoadSurface::Cobbles, 3)]);
    }

    #[test]
    fn a_street_cobbled_in_part_is_split_where_its_surface_changes() {
        let road = street(&[(0.0, 0.0), (40.0, 0.0), (80.0, 0.0), (120.0, 0.0)]);
        let sett = way(
            1,
            RoadKind::Local,
            RoadSurface::Cobbles,
            &[(0.0, 0.0), (80.0, 0.0)],
        );

        assert_eq!(
            surfaced(vec![road], &[sett]),
            [(RoadSurface::Cobbles, 3), (RoadSurface::Paved, 2)]
        );
    }

    #[test]
    fn a_paved_sidewalk_beside_an_asphalt_street_leaves_the_street_as_it_is() {
        let road = street(&[(0.0, 0.0), (100.0, 0.0)]);
        let sidewalk = way(
            1,
            RoadKind::Path,
            RoadSurface::Paving,
            &[(0.0, 3.0), (100.0, 3.0)],
        );
        // A street of the same kind beside it is another street.
        let lane = way(
            2,
            RoadKind::Local,
            RoadSurface::Paving,
            &[(0.0, 6.0), (100.0, 6.0)],
        );

        assert_eq!(
            surfaced(vec![road], &[sidewalk, lane]),
            [(RoadSurface::Paved, 2)]
        );
    }

    #[test]
    fn a_crossing_way_or_a_surface_the_tiles_deny_changes_nothing() {
        let road = street(&[(0.0, 0.0), (100.0, 0.0)]);
        let crossing = way(
            1,
            RoadKind::Local,
            RoadSurface::Cobbles,
            &[(50.0, -50.0), (50.0, 50.0)],
        );
        // The tiles say paved: a mapped dirt surface is some other way's.
        let dirt = way(
            2,
            RoadKind::Local,
            RoadSurface::Dirt,
            &[(0.0, 0.0), (100.0, 0.0)],
        );

        assert_eq!(
            surfaced(vec![road], &[crossing, dirt]),
            [(RoadSurface::Paved, 2)]
        );
    }

    #[test]
    fn a_few_metres_of_another_surface_leave_no_sliver() {
        let run = |surface, start, length| Run {
            surface,
            start,
            end: start + 1,
            length,
        };
        let sett = Some(RoadSurface::Cobbles);
        let dirt = Some(RoadSurface::Dirt);
        let joined = |runs: Vec<Run>| -> Vec<_> {
            without_slivers(runs)
                .iter()
                .map(|r| (r.surface, r.start, r.end))
                .collect()
        };
        // Within a run, at its start and at its end.
        assert_eq!(
            joined(vec![
                run(sett, 0, 50.0),
                run(None, 1, 3.0),
                run(sett, 2, 50.0)
            ]),
            [(sett, 0, 3)]
        );
        assert_eq!(
            joined(vec![run(None, 0, 3.0), run(sett, 1, 50.0)]),
            [(sett, 0, 2)]
        );
        assert_eq!(
            joined(vec![run(sett, 0, 50.0), run(None, 1, 3.0)]),
            [(sett, 0, 2)]
        );
        // Long enough, each keeps its own.
        assert_eq!(
            joined(vec![
                run(sett, 0, 50.0),
                run(None, 1, 6.0),
                run(dirt, 2, 50.0)
            ]),
            [(sett, 0, 1), (None, 1, 2), (dirt, 2, 3)]
        );
    }

    #[test]
    fn without_detail_the_roads_stay_as_they_are() {
        let roads = vec![street(&[(0.0, 0.0), (100.0, 0.0)])];
        let mut same = roads.clone();
        surfaces(&mut same, &[]);
        assert_eq!(same, roads);
    }
}
