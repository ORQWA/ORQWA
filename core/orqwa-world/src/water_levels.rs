//! Water elevations shared by every chunk, independent of road shaping and bank terrain.

use std::collections::HashMap;
use std::sync::Arc;

use orqwa_routes::{ElevationModel, LocalProjection};

use crate::road::RoadIndex;
use crate::water::{Pool, Stream};
use crate::{channels, drape};

const CELL: f64 = 64.0;
const STEP: f64 = 10.0;
const BANK_REACH: f64 = channels::BANK_REACH;

type Point = (f64, f64);

/// The longitudinal water profiles and their footprints, indexed independently of chunks.
#[derive(Default)]
pub(crate) struct Levels {
    surfaces: Vec<Surface>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

struct Surface {
    outline: Vec<Point>,
    half: f64,
    profiles: Vec<Arc<Profile>>,
}

struct Profile {
    samples: Vec<(Point, f64)>,
    cells: HashMap<(i64, i64), Vec<usize>>,
}

impl Levels {
    pub(crate) async fn new<M: ElevationModel>(
        streams: &[Stream],
        pools: &[Pool],
        projection: &LocalProjection,
        road: &RoadIndex,
        model: &mut M,
    ) -> Arc<Self> {
        let mut levels = Self::default();
        let mut stream_profiles = Vec::with_capacity(streams.len());
        for stream in streams {
            stream_profiles.push(Arc::new(
                Profile::new(stream.points(), projection, road, model).await,
            ));
        }
        // Areas take precedence over the narrow symbolic stream strips on the same river.
        for pool in pools {
            let mut profiles = Vec::new();
            for (stream, profile) in streams.iter().zip(&stream_profiles) {
                if stream.points().iter().any(|&p| contains(pool.outline(), p)) {
                    profiles.push(Arc::clone(profile));
                }
            }
            if profiles.is_empty() {
                let line = middle(pool.outline());
                profiles.push(Arc::new(Profile::new(&line, projection, road, model).await));
            }
            levels.insert(Surface {
                outline: pool.outline().to_vec(),
                half: 0.0,
                profiles,
            });
        }
        for (stream, profile) in streams.iter().zip(stream_profiles) {
            levels.insert(Surface {
                outline: stream.points().to_vec(),
                half: stream.width() / 2.0,
                profiles: vec![profile],
            });
        }
        Arc::new(levels)
    }

    fn insert(&mut self, surface: Surface) {
        let reach = surface.half + BANK_REACH;
        let low = surface
            .outline
            .iter()
            .fold((f64::INFINITY, f64::INFINITY), |a, p| {
                (a.0.min(p.0), a.1.min(p.1))
            });
        let high = surface
            .outline
            .iter()
            .fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| {
                (a.0.max(p.0), a.1.max(p.1))
            });
        let index = self.surfaces.len();
        for x in cell(low.0 - reach)..=cell(high.0 + reach) {
            for y in cell(low.1 - reach)..=cell(high.1 + reach) {
                self.cells.entry((x, y)).or_default().push(index);
            }
        }
        self.surfaces.push(surface);
    }

    /// The uncarved water datum, at and just outside a mapped water footprint.
    pub(crate) fn at(&self, east: f64, north: f64) -> Option<f64> {
        let p = (east, north);
        let mut nearby = None;
        let mut distance = f64::INFINITY;
        for &index in self.cells.get(&(cell(east), cell(north)))? {
            let surface = &self.surfaces[index];
            let inside = surface.half == 0.0 && contains(&surface.outline, p);
            let gap = if inside { 0.0 } else { surface.distance(p) };
            if gap < distance && gap <= BANK_REACH {
                nearby = surface.level(p);
                distance = gap;
                if inside {
                    break;
                }
            }
        }
        nearby
    }
}

impl Surface {
    fn distance(&self, p: Point) -> f64 {
        if self.half > 0.0 {
            return self.profiles[0]
                .at(p)
                .map_or(f64::INFINITY, |sample| (sample.0 - self.half).max(0.0));
        }
        let mut distance = self
            .outline
            .windows(2)
            .map(|pair| project(p, pair[0], pair[1]).0)
            .fold(f64::INFINITY, f64::min);
        if self.half == 0.0 {
            distance =
                distance.min(project(p, self.outline[self.outline.len() - 1], self.outline[0]).0);
        }
        (distance - self.half).max(0.0)
    }

    fn level(&self, p: Point) -> Option<f64> {
        self.profiles
            .iter()
            .filter_map(|profile| profile.at(p))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, height)| height)
    }
}

impl Profile {
    async fn new<M: ElevationModel>(
        line: &[Point],
        projection: &LocalProjection,
        road: &RoadIndex,
        model: &mut M,
    ) -> Self {
        let mut samples = Vec::new();
        for p in drape::densify(line, STEP) {
            let (lat, lon) = projection.unproject(p.0, p.1);
            let height = model
                .elevation(lat, lon)
                .await
                .ok()
                .filter(|h| h.is_finite())
                .or_else(|| road.nearest(p.0, p.1, f64::INFINITY).map(|r| r.1))
                .unwrap_or(0.0);
            samples.push((p, height));
        }
        // A river may descend, but cannot climb back over a DEM bump at a bridge or bank.
        // Isotonic regression pools conflicting samples without turning one low outlier into
        // a level reach all the way downstream. Direction comes from heights, not map order.
        if samples.len() > 1 {
            let sign = if samples[0].1 <= samples[samples.len() - 1].1 {
                1.0
            } else {
                -1.0
            };
            let mut blocks: Vec<(f64, usize)> = Vec::new();
            for &(_, height) in &samples {
                blocks.push((height * sign, 1));
                while blocks.len() > 1 {
                    let last = blocks.len() - 1;
                    let (a, na) = blocks[last - 1];
                    let (b, nb) = blocks[last];
                    #[allow(clippy::cast_precision_loss)] // small sample counts
                    let ordered = a / na as f64 <= b / nb as f64;
                    if ordered {
                        break;
                    }
                    blocks.pop();
                    blocks[last - 1] = (a + b, na + nb);
                }
            }
            let mut offset = 0;
            for (sum, count) in blocks {
                #[allow(clippy::cast_precision_loss)] // small sample counts
                let height = sign * sum / count as f64;
                for sample in &mut samples[offset..offset + count] {
                    sample.1 = height;
                }
                offset += count;
            }
        }
        let mut cells: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (index, pair) in samples.windows(2).enumerate() {
            for x in cell(pair[0].0.0.min(pair[1].0.0))..=cell(pair[0].0.0.max(pair[1].0.0)) {
                for y in cell(pair[0].0.1.min(pair[1].0.1))..=cell(pair[0].0.1.max(pair[1].0.1)) {
                    cells.entry((x, y)).or_default().push(index);
                }
            }
        }
        Self { samples, cells }
    }

    fn at(&self, p: Point) -> Option<(f64, f64)> {
        if self.samples.len() == 1 {
            return Some((
                (p.0 - self.samples[0].0.0).hypot(p.1 - self.samples[0].0.1),
                self.samples[0].1,
            ));
        }
        let sample = |index: usize| {
            let pair = &self.samples[index..index + 2];
            let (distance, t) = project(p, pair[0].0, pair[1].0);
            (distance, pair[0].1 + (pair[1].1 - pair[0].1) * t)
        };
        let (x, y) = (cell(p.0), cell(p.1));
        let nearest = (x - 1..=x + 1)
            .flat_map(|cx| (y - 1..=y + 1).filter_map(move |cy| self.cells.get(&(cx, cy))))
            .flatten()
            .map(|&index| sample(index))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        #[allow(clippy::cast_precision_loss)] // local grid coordinates
        let boundary = (p.0 - (x - 1) as f64 * CELL)
            .min((x + 2) as f64 * CELL - p.0)
            .min(p.1 - (y - 1) as f64 * CELL)
            .min((y + 2) as f64 * CELL - p.1);
        if nearest.is_some_and(|candidate| candidate.0 <= boundary) {
            return nearest;
        }
        (0..self.samples.len().saturating_sub(1))
            .map(sample)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }
}

/// Without a mapped centre line, narrow areas get cross-sections along their principal axis;
/// broad pools get one interior datum, so a lake never tilts with the surrounding hills.
fn middle(outline: &[Point]) -> Vec<Point> {
    #[allow(clippy::cast_precision_loss)] // polygon vertex count
    let count = outline.len() as f64;
    let mean = outline
        .iter()
        .fold((0.0, 0.0), |s, p| (s.0 + p.0 / count, s.1 + p.1 / count));
    let (xx, xy, yy) = outline.iter().fold((0.0, 0.0, 0.0), |s, p| {
        let (x, y) = (p.0 - mean.0, p.1 - mean.1);
        (s.0 + x * x, s.1 + x * y, s.2 + y * y)
    });
    let angle = 0.5 * (2.0 * xy).atan2(xx - yy);
    let axis = (angle.cos(), angle.sin());
    let along = |p: Point| (p.0 - mean.0) * axis.0 + (p.1 - mean.1) * axis.1;
    let across = |p: Point| -(p.0 - mean.0) * axis.1 + (p.1 - mean.1) * axis.0;
    let (mut low, mut high, mut left, mut right) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for &p in outline {
        low = low.min(along(p));
        high = high.max(along(p));
        left = left.min(across(p));
        right = right.max(across(p));
    }
    let narrow = high - low > (right - left) * 4.0;
    let mut line = Vec::new();
    let mut position = if narrow {
        low + 0.01
    } else {
        low.midpoint(high)
    };
    while position < high {
        let mut intersections = Vec::new();
        for (i, &a) in outline.iter().enumerate() {
            let b = outline[(i + 1) % outline.len()];
            let (start, end) = (along(a), along(b));
            if (start <= position && end > position) || (end <= position && start > position) {
                let t = (position - start) / (end - start);
                intersections.push(across(a) + (across(b) - across(a)) * t);
            }
        }
        intersections.sort_by(f64::total_cmp);
        if let Some(pair) = intersections
            .as_chunks::<2>()
            .0
            .iter()
            .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        {
            let side = pair[0].midpoint(pair[1]);
            line.push((
                mean.0 + axis.0 * position - axis.1 * side,
                mean.1 + axis.1 * position + axis.0 * side,
            ));
        }
        if !narrow || position >= high - 0.01 {
            break;
        }
        position = (position + STEP).min(high - 0.01);
    }
    line
}

fn contains(outline: &[Point], p: Point) -> bool {
    let mut inside = false;
    for (i, &a) in outline.iter().enumerate() {
        let b = outline[(i + 1) % outline.len()];
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
    }
    inside
}

fn project(p: Point, a: Point, b: Point) -> (f64, f64) {
    let (east, north) = (b.0 - a.0, b.1 - a.1);
    let share = (((p.0 - a.0) * east + (p.1 - a.1) * north)
        / (east * east + north * north).max(1e-12))
    .clamp(0.0, 1.0);
    (
        (p.0 - a.0 - east * share).hypot(p.1 - a.1 - north * share),
        share,
    )
}

#[allow(clippy::cast_possible_truncation)] // local world coordinates
fn cell(value: f64) -> i64 {
    (value / CELL).floor() as i64
}
