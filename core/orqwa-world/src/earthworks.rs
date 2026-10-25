//! Inferred road beds and earthworks from saved terrain, shared by every terrain chunk.

use std::collections::HashMap;

use orqwa_routes::{ElevationModel, LocalProjection};

use crate::{MeshData, streets};

const SMOOTH_M: f64 = 24.0;
const GRADE: f64 = 0.20;
const WALL_START: f64 = 2.5;
const WALL_END: f64 = 6.0;

/// Cache the global terrain lattice; profiling dense road networks must not resample it
/// for every three-metre road station.
#[derive(Default)]
pub(crate) struct Samples {
    heights: HashMap<(i64, i64), Option<f64>>,
    junctions: HashMap<(i64, i64), Vec<(f64, f64)>>,
}

impl Samples {
    pub(crate) fn new(lines: &[Vec<(f64, f64)>]) -> Self {
        let mut samples = Self::default();
        // Tag changes and branch endpoints keep the same natural datum on every arm.
        for line in lines {
            for &point in line.first().into_iter().chain(line.last()) {
                samples
                    .junctions
                    .entry(junction_cell(point))
                    .or_default()
                    .push(point);
            }
        }
        samples
    }

    pub(crate) async fn profile<M: ElevationModel>(
        &mut self,
        points: &[(f64, f64)],
        projection: &LocalProjection,
        model: &mut M,
    ) -> Option<Vec<f64>> {
        let mut heights = Vec::with_capacity(points.len());
        let mut distances = vec![0.0];
        for (i, &(east, north)) in points.iter().enumerate() {
            let (u, v) = (east / crate::GRID, north / crate::GRID);
            #[allow(clippy::cast_possible_truncation)] // local metre coordinates
            let (x, y) = (u.floor() as i64, v.floor() as i64);
            let mut corners = [0.0; 4];
            for (slot, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                let key = (x + dx, y + dy);
                if let std::collections::hash_map::Entry::Vacant(entry) = self.heights.entry(key) {
                    #[allow(clippy::cast_precision_loss)] // terrain lattice coordinates
                    let (lat, lon) = projection
                        .unproject(key.0 as f64 * crate::GRID, key.1 as f64 * crate::GRID);
                    entry.insert(model.elevation(lat, lon).await.ok());
                }
                corners[slot] = self.heights[&key]?;
            }
            let (fu, fv) = (u - u.floor(), v - v.floor());
            let [sw, se, nw, ne] = corners;
            heights.push((sw + (se - sw) * fu) * (1.0 - fv) + (nw + (ne - nw) * fu) * fv);
            if i > 0 {
                let previous = points[i - 1];
                distances.push(distances[i - 1] + (east - previous.0).hypot(north - previous.1));
            }
        }
        if let (Some(first), Some(last)) = (points.first(), points.last())
            && (last.0 - first.0).hypot(last.1 - first.1) < 0.5
        {
            // Roundabouts already have a shared circular datum and raised island.
            return Some(heights);
        }
        let mut anchors = vec![0];
        for (i, &point) in points.iter().enumerate().skip(1) {
            let (x, y) = junction_cell(point);
            let junction = (x - 1..=x + 1).any(|x| {
                (y - 1..=y + 1).any(|y| {
                    self.junctions
                        .get(&(x, y))
                        .into_iter()
                        .flatten()
                        .any(|p| (p.0 - point.0).hypot(p.1 - point.1) < 0.5)
                })
            });
            if junction || i == points.len() - 1 {
                anchors.push(i);
            }
        }
        let mut profile = heights.clone();
        for pair in anchors.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            profile[start..=end]
                .copy_from_slice(&smooth(&heights[start..=end], &distances[start..=end]));
        }
        Some(profile)
    }
}

/// Preserve connected chain ends, remove local DEM humps and constrain intermediate grades.
/// Where the endpoints themselves require a steeper grade, preserve that connection rather
/// than inventing a disconnected road or changing the mapped alignment.
fn smooth(raw: &[f64], distances: &[f64]) -> Vec<f64> {
    if raw.len() < 2 {
        return raw.to_vec();
    }
    let last = raw.len() - 1;
    let start = distances[0];
    let total = (distances[last] - start).max(1e-9);
    let grade = GRADE.max((raw[last] - raw[0]).abs() / total);
    let mut smoothed = Vec::with_capacity(raw.len());
    for (i, &station) in distances.iter().enumerate() {
        let distance = station - start;
        let mut sum = 0.0;
        let mut weight = 0.0;
        let (mut offset_sum, mut square_sum, mut height_offset_sum) = (0.0, 0.0, 0.0);
        let mut j = distances.partition_point(|&d| d < station - SMOOTH_M);
        while j < raw.len() && distances[j] - station < SMOOTH_M {
            let offset = distances[j] - station;
            let w = (1.0 - offset.abs() / SMOOTH_M).max(0.0);
            sum += raw[j] * w;
            weight += w;
            offset_sum += offset * w;
            square_sum += offset * offset * w;
            height_offset_sum += raw[j] * offset * w;
            j += 1;
        }
        // A local line preserves an ordinary climb even where the smoothing window ends.
        let variance = square_sum - offset_sum * offset_sum / weight;
        let gradient = if variance > 1e-9 {
            (height_offset_sum - sum * offset_sum / weight) / variance
        } else {
            0.0
        };
        let estimate = (sum - gradient * offset_sum) / weight;
        let blend = (distance.min(total - distance) / SMOOTH_M).clamp(0.0, 1.0);
        let height = raw[i] + (estimate - raw[i]) * blend;
        let low = (raw[0] - grade * distance).max(raw[last] - grade * (total - distance));
        let high = (raw[0] + grade * distance).min(raw[last] + grade * (total - distance));
        smoothed.push(height.clamp(low.min(high), high));
    }
    let mut low = smoothed.clone();
    let mut high = smoothed;
    for i in 1..raw.len() {
        let rise = grade * (distances[i] - distances[i - 1]);
        low[i] = low[i].min(low[i - 1] + rise);
        high[i] = high[i].max(high[i - 1] - rise);
    }
    for i in (0..last).rev() {
        let rise = grade * (distances[i + 1] - distances[i]);
        low[i] = low[i].min(low[i + 1] + rise);
        high[i] = high[i].max(high[i + 1] - rise);
    }
    low.into_iter()
        .zip(high)
        .map(|(a, b)| f64::midpoint(a, b))
        .collect()
}

/// A short earth bank meets a battered retaining face on steep hillsides. Both use the
/// same shaped terrain, avoiding a wall floating above or intersecting a separate cutout.
pub(crate) fn shape(natural: f64, level: f64, beyond: f64) -> f64 {
    let room = (beyond - streets::LEVEL_VERGE_M).max(0.0);
    let earth = natural.clamp(
        level - room * crate::FILL_SLOPE,
        level + room * crate::CUT_SLOPE,
    );
    let fade = ((room - (streets::LEVEL_REACH_M - streets::LEVEL_FADE_M)) / streets::LEVEL_FADE_M)
        .clamp(0.0, 1.0);
    let earth = earth + (natural - earth) * ease(fade);
    let slope = (natural - level).abs() / (beyond + 3.0).max(1.0);
    let retaining = ease(((slope - 0.65) / 0.4).clamp(0.0, 1.0));
    let foot = natural.clamp(
        level - room.min(WALL_START) * crate::FILL_SLOPE,
        level + room.min(WALL_START) * crate::CUT_SLOPE,
    );
    let wall =
        foot + (natural - foot) * ((room - WALL_START) / (WALL_END - WALL_START)).clamp(0.0, 1.0);
    earth + (wall - earth) * retaining
}

fn ease(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

#[allow(clippy::cast_possible_truncation)] // local metre coordinates
fn junction_cell(point: (f64, f64)) -> (i64, i64) {
    (
        (point.0 / 0.5).floor() as i64,
        (point.1 / 0.5).floor() as i64,
    )
}

/// Dress steep engineered banks with cast concrete, following the final terrain triangles
/// exactly, including chunk edges, intersecting roads, carved water and tunnel openings.
pub(crate) fn retaining_faces(
    terrain: &MeshData,
    origin: [f32; 3],
    levels: &streets::Levels,
    heights: &crate::HeightGrid,
) -> MeshData {
    let mut mesh = MeshData::default();
    if levels.is_empty() {
        return mesh;
    }
    for face in terrain.indices.as_chunks::<3>().0 {
        let corners = face.map(|i| terrain.vertices[i as usize].map(f64::from));
        let a = [0, 1, 2].map(|k| corners[1][k] - corners[0][k]);
        let b = [0, 1, 2].map(|k| corners[2][k] - corners[0][k]);
        let cross = [
            a[2] * b[1] - a[1] * b[2],
            a[0] * b[2] - a[2] * b[0],
            a[1] * b[0] - a[0] * b[1],
        ];
        let length = cross[0].hypot(cross[1]).hypot(cross[2]);
        if length < 1e-9 || cross[1] / length > 0.52 {
            continue;
        }
        let centre = [0, 1, 2].map(|k| (corners[0][k] + corners[1][k] + corners[2][k]) / 3.0);
        let (east, north) = (
            centre[0] + f64::from(origin[0]),
            -centre[2] - f64::from(origin[2]),
        );
        let Some(level) = levels.nearest(east, north, 0.0) else {
            continue;
        };
        let room = level.distance - level.half - streets::LEVEL_VERGE_M;
        if !(WALL_START - crate::FINE..=WALL_END + crate::FINE).contains(&room) {
            continue;
        }
        let bed = level
            .height
            .unwrap_or_else(|| heights.natural_at(level.foot.0, level.foot.1));
        let slope = (heights.natural_at(east, north) - bed).abs()
            / (level.distance - level.half + 3.0).max(1.0);
        if slope < 0.75 {
            continue;
        }
        let normal = cross.map(|n| n / length);
        let corners = corners.map(|p| [0, 1, 2].map(|k| p[k] + normal[k] * 0.025));
        crate::structures::triangle_uv(
            &mut mesh,
            corners,
            [[0.0; 2]; 3],
            normal,
            crate::palette::srgb("structure.concrete", 0.0),
        );
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_climbs_retain_their_grade_including_at_junction_ends() {
        let distances: Vec<_> = (0..=50).map(|i| 480.0 + f64::from(i) * 2.8).collect();
        let raw: Vec<_> = distances.iter().map(|&d| 500.0 + 0.12 * d).collect();
        let profile = smooth(&raw, &distances);
        assert!(profile.iter().zip(raw).all(|(a, b)| (a - b).abs() < 1e-9));
    }

    #[test]
    fn local_humps_have_driveable_grades_and_connected_ends() {
        let distances: Vec<_> = (0..=100).map(|i| f64::from(i) * 3.0).collect();
        let raw: Vec<_> = distances
            .iter()
            .map(|&d| 500.0 + 0.05 * d + if (d - 150.0).abs() < 12.0 { 8.0 } else { 0.0 })
            .collect();
        let profile = smooth(&raw, &distances);
        assert!((profile[0] - raw[0]).abs() < 1e-9);
        assert!((profile[100] - raw[100]).abs() < 1e-9);
        assert!(
            profile
                .windows(2)
                .all(|p| (p[1] - p[0]).abs() <= 3.0 * GRADE + 1e-9)
        );
        assert!(profile[50] < raw[50] - 3.0);
    }
}
