//! Inferred urban paving and building access (R105). Geometry follows the drawn terrain;
//! mapped green spaces and infrastructure take precedence over inferred surroundings.

use std::collections::HashMap;

use orqwa_osm::LandCover;
use orqwa_routes::Surface;

use crate::buildings::{self, Plot, Point, Setting};
use crate::landcover::LandIndex;
use crate::road::RoadIndex;
use crate::streets::{Clearance, Street};
use crate::{CHUNK_SIZE, HeightGrid, MeshData, On, ROAD_HALF_WIDTH, ROAD_SINK, drape, palette};

const CELL: f64 = 50.0;
const WALK_WIDTH: f64 = 1.8;
const CURB_HEIGHT: f64 = 0.14;
const GAP: f64 = 0.12;
const ACCESS_REACH: f64 = 35.0;
const DENSE_RADIUS: f64 = 50.0;
const DENSE_COVERAGE: f64 = 0.24;

/// Material codes in the urban mesh's vertex colour, separate from terrain's cover colours.
#[derive(Clone, Copy, PartialEq)]
enum Material {
    Paving,
    Concrete,
    Asphalt,
    Curb,
}

impl Material {
    fn color(self) -> [f32; 4] {
        let code = match self {
            Self::Paving => 0.0,
            Self::Concrete => 0.25,
            Self::Asphalt => 0.5,
            Self::Curb => 0.75,
        };
        [code, 0.0, 0.0, 1.0]
    }
}

struct Site {
    outline: Vec<Point>,
    center: Point,
    area: f64,
    business: bool,
    town: bool,
}

#[derive(Clone, Copy)]
struct Way {
    a: Point,
    b: Point,
    half: f64,
    lift: f64,
}

struct Patch {
    polygon: Vec<Point>,
    material: Material,
    lift: f64,
    /// Sidewalks have a vertical curb on their first edge. Boundary walls rise instead.
    curb: bool,
    wall: bool,
    /// Access paths rise gently to meet their street and the lowered sidewalk opening.
    ramp: Option<(Point, Point, f64)>,
}

/// Immutable, spatially indexed urban surfaces, shared by all chunk builders.
#[derive(Default)]
pub(crate) struct Urban {
    sites: Vec<Site>,
    site_cells: HashMap<(i64, i64), Vec<usize>>,
    building_cells: HashMap<(i64, i64), Vec<usize>>,
    ways: Vec<Way>,
    way_cells: HashMap<(i64, i64), Vec<usize>>,
    patches: Vec<Patch>,
    patch_cells: HashMap<(i64, i64), Vec<usize>>,
    chunk_cells: HashMap<(i64, i64), Vec<usize>>,
}

// Kept outside the builder so its decisions can be queried identically by terrain and plants.
impl Urban {
    pub(crate) fn build<'a>(
        plots: impl Iterator<Item = &'a Plot<'a>>,
        streets: &[Street],
        road: &RoadIndex,
        land: &LandIndex,
    ) -> Self {
        let mut plots: Vec<_> = plots.collect();
        plots.sort_by_key(|plot| plot.building.id);
        let mut urban = Self::default();
        for plot in plots {
            let outline = buildings::drawn_outline(&plot.footprint);
            let center = centroid(&outline);
            let area = polygon_area(&outline);
            let index = urban.sites.len();
            urban
                .site_cells
                .entry(cell(center, CELL))
                .or_default()
                .push(index);
            for key in cells(&outline, CELL, 0.0) {
                urban.building_cells.entry(key).or_default().push(index);
            }
            urban.sites.push(Site {
                outline,
                center,
                area,
                business: plot.shop.is_some()
                    || plot.purpose.is_some()
                    || matches!(
                        plot.setting,
                        Setting::Commercial | Setting::Industrial | Setting::Public
                    ),
                town: matches!(plot.setting, Setting::Town),
            });
        }
        urban.collect_ways(streets, road);
        urban.walks(land);
        urban.accesses(land);
        urban
    }

    fn collect_ways(&mut self, streets: &[Street], road: &RoadIndex) {
        for street in streets.iter().filter(|s| s.paved() && !s.on_bridge()) {
            for pair in street.points().windows(2) {
                let direction = direction(pair[0], pair[1]);
                let middle = midpoint(pair[0], pair[1]);
                if road.runs_along(
                    middle.0,
                    middle.1,
                    ROAD_HALF_WIDTH + street.half_width() + 0.5,
                    direction,
                ) {
                    continue;
                }
                self.add_way(Way {
                    a: pair[0],
                    b: pair[1],
                    half: street.half_width(),
                    lift: street.lift(),
                });
            }
        }
        let stations = road.stations(3.0, (f64::MIN, f64::MIN), (f64::MAX, f64::MAX));
        for pair in stations.windows(2) {
            if pair.iter().any(|s| s.surface != Surface::Ground)
                || pair[1].distance - pair[0].distance > 3.1
            {
                continue;
            }
            self.add_way(Way {
                a: pair[0].position,
                b: pair[1].position,
                half: ROAD_HALF_WIDTH,
                lift: ROAD_SINK,
            });
        }
    }

    fn add_way(&mut self, way: Way) {
        if distance(way.a, way.b) < 0.01 {
            return;
        }
        let index = self.ways.len();
        for key in cells(&[way.a, way.b], CELL, 0.0) {
            self.way_cells.entry(key).or_default().push(index);
        }
        self.ways.push(way);
    }

    fn nearest_way(&self, point: Point) -> Option<(Point, Way)> {
        Self::near(&self.way_cells, point, ACCESS_REACH)
            .map(|index| self.ways[index])
            .map(|way| (project(point, way.a, way.b), way))
            .filter(|(p, _)| distance(point, *p) < ACCESS_REACH)
            .min_by(|a, b| distance(point, a.0).total_cmp(&distance(point, b.0)))
    }

    fn near(
        index: &HashMap<(i64, i64), Vec<usize>>,
        point: Point,
        reach: f64,
    ) -> impl Iterator<Item = usize> + '_ {
        let keys = cells(&[point], CELL, reach);
        keys.into_iter()
            .flat_map(|key| index.get(&key).into_iter().flatten().copied())
    }

    fn dense(&self, point: Point) -> bool {
        let area: f64 = Self::near(&self.site_cells, point, DENSE_RADIUS)
            .map(|index| &self.sites[index])
            .filter(|site| distance(point, site.center) < DENSE_RADIUS)
            .map(|site| site.area)
            .sum();
        area > std::f64::consts::PI * DENSE_RADIUS.powi(2) * DENSE_COVERAGE
    }

    fn urban_at(&self, point: Point, land: &LandIndex) -> bool {
        !protected(point, land)
            && (matches!(
                land.cover_at(point.0, point.1),
                Some(
                    LandCover::Residential
                        | LandCover::Commercial
                        | LandCover::Industrial
                        | LandCover::Public
                )
            ) || self.dense(point))
    }

    fn walks(&mut self, land: &LandIndex) {
        for index in 0..self.ways.len() {
            let way = self.ways[index];
            if way.half < 2.5 || !self.urban_at(midpoint(way.a, way.b), land) {
                continue;
            }
            let forward = direction(way.a, way.b);
            let right = (forward.1, -forward.0);
            for sign in [-1.0, 1.0] {
                let offset = |p: Point, width: f64| {
                    (p.0 + right.0 * width * sign, p.1 + right.1 * width * sign)
                };
                let inner = way.half + GAP;
                self.add_patch(Patch {
                    polygon: vec![
                        offset(way.a, inner),
                        offset(way.b, inner),
                        offset(way.b, inner + WALK_WIDTH),
                        offset(way.a, inner + WALK_WIDTH),
                    ],
                    material: Material::Concrete,
                    lift: way.lift + CURB_HEIGHT,
                    curb: true,
                    wall: false,
                    ramp: None,
                });
            }
        }
    }

    fn accesses(&mut self, land: &LandIndex) {
        for index in 0..self.sites.len() {
            let site = &self.sites[index];
            if !self.urban_at(site.center, land) || site.outline.len() < 3 {
                continue;
            }
            let Some((street, way)) = self.nearest_way(site.center) else {
                continue;
            };
            let edge = (0..site.outline.len())
                .map(|k| (site.outline[k], site.outline[(k + 1) % site.outline.len()]))
                .min_by(|a, b| {
                    distance(street, midpoint(a.0, a.1))
                        .total_cmp(&distance(street, midpoint(b.0, b.1)))
                })
                .expect("a building outline");
            let door = midpoint(edge.0, edge.1);
            let reach = distance(door, street);
            if reach <= way.half + GAP || reach > ACCESS_REACH {
                continue;
            }
            let to_street = direction(door, street);
            let end = (
                street.0 - to_street.0 * (way.half + GAP),
                street.1 - to_street.1 * (way.half + GAP),
            );
            let business = site.business;
            let town = site.town;
            let area = site.area;
            let front = direction(edge.0, edge.1);
            let apron_reach = if business { 3.0 } else { 0.9 };
            let outer = |p: Point| {
                (
                    p.0 + to_street.0 * apron_reach,
                    p.1 + to_street.1 * apron_reach,
                )
            };
            let apron = vec![edge.0, edge.1, outer(edge.1), outer(edge.0)];
            self.add_patch(Patch {
                polygon: apron,
                material: Material::Paving,
                lift: 0.025,
                curb: false,
                wall: false,
                ramp: None,
            });
            let half = if business { 1.7 } else { 0.75 };
            self.add_patch(Patch {
                polygon: ribbon(door, end, half),
                material: if business {
                    Material::Asphalt
                } else {
                    Material::Paving
                },
                lift: 0.025,
                curb: false,
                wall: false,
                ramp: Some((door, end, way.lift + 0.005)),
            });
            // Small front-garden walls leave a broad opening for the access, inferred from
            // the house and road rather than pretending cadastral boundaries are mapped.
            if town && !business && area < 300.0 && reach > way.half + WALK_WIDTH + 4.0 {
                let at = (
                    end.0 - to_street.0 * (WALK_WIDTH + 0.7),
                    end.1 - to_street.1 * (WALK_WIDTH + 0.7),
                );
                for sign in [-1.0, 1.0] {
                    let a = (at.0 + front.0 * sign * 1.5, at.1 + front.1 * sign * 1.5);
                    let b = (at.0 + front.0 * sign * 3.8, at.1 + front.1 * sign * 3.8);
                    self.add_patch(Patch {
                        polygon: ribbon(a, b, 0.12),
                        material: Material::Concrete,
                        lift: 0.65,
                        curb: false,
                        wall: true,
                        ramp: None,
                    });
                }
            }
        }
    }

    fn add_patch(&mut self, mut patch: Patch) {
        if signed_area(&patch.polygon) > 0.0 {
            // drape_polygon expects clockwise corners; preserve the curb's first edge.
            patch.polygon.reverse();
            if patch.curb {
                patch.polygon.rotate_left(2);
            }
        }
        let index = self.patches.len();
        for key in cells(&patch.polygon, CELL, 0.0) {
            self.patch_cells.entry(key).or_default().push(index);
        }
        for key in cells(&patch.polygon, CHUNK_SIZE, 0.0) {
            self.chunk_cells.entry(key).or_default().push(index);
        }
        self.patches.push(patch);
    }

    /// Whether plants would stand in inferred paving. Mapped green land always wins.
    pub(crate) fn paved_at(&self, point: Point, land: &LandIndex) -> bool {
        if protected(point, land) {
            return false;
        }
        if matches!(
            land.cover_at(point.0, point.1),
            Some(LandCover::Commercial | LandCover::Industrial)
        ) {
            return true;
        }
        if self.dense(point)
            && matches!(
                land.cover_at(point.0, point.1),
                Some(LandCover::Residential) | None
            )
        {
            return true;
        }
        self.patch_cells
            .get(&cell(point, CELL))
            .into_iter()
            .flatten()
            .map(|&index| &self.patches[index])
            .any(|patch| contains(&patch.polygon, point))
    }

    /// Terrain between dense blocks is paved, while suburban gardens retain their lawns.
    pub(crate) fn paint(&self, mesh: &mut MeshData, origin: [f64; 3], land: &LandIndex) {
        for (v, color) in mesh.vertices.iter().zip(&mut mesh.colors) {
            let point = (f64::from(v[0]) + origin[0], -f64::from(v[2]) - origin[2]);
            if !protected(point, land)
                && matches!(
                    land.cover_at(point.0, point.1),
                    Some(LandCover::Residential) | None
                )
                && self.dense(point)
            {
                *color = palette::srgb("ground.urban", 0.0);
            }
        }
    }

    /// Height above terrain of a clear sidewalk, excluding entrance paths through it.
    pub(crate) fn sidewalk_lift(
        &self,
        point: Point,
        land: &LandIndex,
        clearance: &Clearance,
        road: &RoadIndex,
    ) -> Option<f64> {
        self.patch_cells
            .get(&cell(point, CELL))?
            .iter()
            .map(|&index| &self.patches[index])
            .filter(|patch| {
                patch.curb
                    && contains(&patch.polygon, point)
                    && !self.access_crosses(&patch.polygon, true)
                    && self.patch_clear(patch, land, clearance, road)
            })
            .map(|patch| {
                patch.lift
                    - if self.access_crosses(&patch.polygon, false) {
                        CURB_HEIGHT
                    } else {
                        0.0
                    }
            })
            .max_by(f64::total_cmp)
    }

    fn patch_clear(
        &self,
        patch: &Patch,
        land: &LandIndex,
        clearance: &Clearance,
        road: &RoadIndex,
    ) -> bool {
        let valid = |point: Point| {
            !protected(point, land)
                && !clearance.blocked_by(point.0, point.1, 0.02, |east, north| {
                    // The ridden road is checked precisely below; its map line's padded
                    // point samples otherwise swallow the sidewalk beside it.
                    road.nearest(east, north, ROAD_HALF_WIDTH + 0.5).is_none()
                })
                && road
                    .nearest(point.0, point.1, ROAD_HALF_WIDTH + 0.02)
                    .is_none()
                && !self.in_building(point)
        };
        // Furniture and mesh generation must agree about which sidewalk pieces exist.
        valid(centroid(&patch.polygon))
            && patch
                .polygon
                .iter()
                .zip(patch.polygon.iter().cycle().skip(1))
                .take(patch.polygon.len())
                .flat_map(|(&a, &b)| drape::densify(&[a, b], 1.0))
                .all(valid)
    }

    pub(crate) fn mesh(
        &self,
        heights: &HeightGrid,
        land: &LandIndex,
        clearance: &Clearance,
        road: &RoadIndex,
        origin: [f64; 3],
    ) -> MeshData {
        let mut mesh = MeshData::default();
        for &index in self
            .chunk_cells
            .get(&cell(heights.origin, CHUNK_SIZE))
            .into_iter()
            .flatten()
        {
            let patch = &self.patches[index];
            // Short pieces leave gaps at intersections and keep hard surfaces out of parks,
            // water and neighbouring buildings. Also check edges, not only their centre.
            if !self.patch_clear(patch, land, clearance, road) {
                continue;
            }
            if patch.wall
                && patch
                    .polygon
                    .iter()
                    .map(|p| heights.at(p.0, p.1))
                    .fold(f64::MIN, f64::max)
                    - patch
                        .polygon
                        .iter()
                        .map(|p| heights.at(p.0, p.1))
                        .fold(f64::MAX, f64::min)
                    > 0.4
            {
                continue;
            }
            let access = patch.curb && self.access_crosses(&patch.polygon, false);
            let lift = patch.lift - if access { CURB_HEIGHT } else { 0.0 };
            let first = mesh.vertices.len();
            drape::drape_polygon(
                &mut mesh,
                &patch.polygon,
                (On::Ground, lift),
                heights,
                origin,
                &uv,
            );
            if let Some((start, end, end_lift)) = patch.ramp {
                for vertex in &mut mesh.vertices[first..] {
                    let point = (
                        f64::from(vertex[0]) + origin[0],
                        -f64::from(vertex[2]) - origin[2],
                    );
                    let along = distance(start, project(point, start, end))
                        / distance(start, end).max(0.01);
                    #[allow(clippy::cast_possible_truncation)] // a small offset in GPU geometry
                    {
                        vertex[1] += ((end_lift - lift) * along.powi(4)) as f32;
                    }
                }
            }
            mesh.colors
                .resize(mesh.vertices.len(), patch.material.color());
            if patch.curb && !access {
                vertical(
                    &mut mesh,
                    patch.polygon[0],
                    patch.polygon[1],
                    patch.lift - CURB_HEIGHT,
                    patch.lift,
                    heights,
                    origin,
                );
            } else if patch.wall {
                for k in 0..patch.polygon.len() {
                    vertical(
                        &mut mesh,
                        patch.polygon[k],
                        patch.polygon[(k + 1) % patch.polygon.len()],
                        0.0,
                        patch.lift,
                        heights,
                        origin,
                    );
                }
            }
        }
        mesh
    }

    fn in_building(&self, point: Point) -> bool {
        self.building_cells
            .get(&cell(point, CELL))
            .into_iter()
            .flatten()
            .any(|&index| {
                let outline = &self.sites[index].outline;
                contains(outline, point)
                    && outline
                        .iter()
                        .zip(outline.iter().cycle().skip(1))
                        .take(outline.len())
                        .all(|(&a, &b)| distance(point, project(point, a, b)) > 0.08)
            })
    }

    fn access_crosses(&self, polygon: &[Point], entrances_only: bool) -> bool {
        cells(polygon, CELL, 0.0)
            .into_iter()
            .flat_map(|key| self.patch_cells.get(&key).into_iter().flatten())
            .map(|&index| &self.patches[index])
            .any(|patch| {
                !patch.curb
                    && !patch.wall
                    && (!entrances_only || patch.ramp.is_some())
                    && overlaps(polygon, &patch.polygon)
            })
    }
}

fn protected(point: Point, land: &LandIndex) -> bool {
    [
        LandCover::Forest,
        LandCover::Meadow,
        LandCover::Water,
        LandCover::Rock,
        LandCover::Farmland,
        LandCover::Orchard,
    ]
    .into_iter()
    .any(|cover| land.has(point.0, point.1, cover))
}

fn cell(p: Point, size: f64) -> (i64, i64) {
    #[allow(clippy::cast_possible_truncation)]
    {
        ((p.0 / size).floor() as i64, (p.1 / size).floor() as i64)
    }
}

fn cells(points: &[Point], size: f64, reach: f64) -> Vec<(i64, i64)> {
    let low = points.iter().fold((f64::MAX, f64::MAX), |m, p| {
        (m.0.min(p.0 - reach), m.1.min(p.1 - reach))
    });
    let high = points.iter().fold((f64::MIN, f64::MIN), |m, p| {
        (m.0.max(p.0 + reach), m.1.max(p.1 + reach))
    });
    let (a, b) = (cell(low, size), cell(high, size));
    (a.0..=b.0)
        .flat_map(|x| (a.1..=b.1).map(move |y| (x, y)))
        .collect()
}

fn distance(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}
fn midpoint(a: Point, b: Point) -> Point {
    (a.0.midpoint(b.0), a.1.midpoint(b.1))
}
fn direction(a: Point, b: Point) -> Point {
    let d = distance(a, b).max(1e-9);
    ((b.0 - a.0) / d, (b.1 - a.1) / d)
}
fn project(p: Point, a: Point, b: Point) -> Point {
    let forward = direction(a, b);
    let along = ((p.0 - a.0) * forward.0 + (p.1 - a.1) * forward.1).clamp(0.0, distance(a, b));
    (a.0 + forward.0 * along, a.1 + forward.1 * along)
}
fn centroid(points: &[Point]) -> Point {
    #[allow(clippy::cast_precision_loss)]
    let n = points.len().max(1) as f64;
    (
        points.iter().map(|p| p.0).sum::<f64>() / n,
        points.iter().map(|p| p.1).sum::<f64>() / n,
    )
}
fn signed_area(points: &[Point]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.0 * b.1 - a.1 * b.0)
        .sum::<f64>()
        * 0.5
}
fn polygon_area(points: &[Point]) -> f64 {
    signed_area(points).abs()
}
fn contains(points: &[Point], p: Point) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
    }
    inside
}

fn overlaps(first: &[Point], second: &[Point]) -> bool {
    let edges = |polygon: &[Point]| {
        polygon
            .iter()
            .copied()
            .zip(polygon.iter().copied().cycle().skip(1))
            .take(polygon.len())
            .collect::<Vec<_>>()
    };
    first.iter().any(|&p| contains(second, p))
        || second.iter().any(|&p| contains(first, p))
        || edges(first).into_iter().any(|(a, b)| {
            edges(second).into_iter().any(|(c, d)| {
                let cross = |p: Point, q: Point, r: Point| {
                    (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
                };
                cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0
            })
        })
}
fn ribbon(a: Point, b: Point, half: f64) -> Vec<Point> {
    let d = direction(a, b);
    let offset = (d.1 * half, -d.0 * half);
    vec![
        (a.0 - offset.0, a.1 - offset.1),
        (b.0 - offset.0, b.1 - offset.1),
        (b.0 + offset.0, b.1 + offset.1),
        (a.0 + offset.0, a.1 + offset.1),
    ]
}
#[allow(clippy::cast_possible_truncation)] // world metres to GPU data
fn uv(p: Point) -> [f32; 2] {
    [p.0 as f32, -p.1 as f32]
}

#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)] // draped curb geometry
fn vertical(
    mesh: &mut MeshData,
    a: Point,
    b: Point,
    low: f64,
    high: f64,
    heights: &HeightGrid,
    origin: [f64; 3],
) {
    let high_corner = (heights.origin.0 + CHUNK_SIZE, heights.origin.1 + CHUNK_SIZE);
    for piece in drape::pieces(&[a, b], heights.origin, high_corner) {
        let (a, b) = (piece[0].0, piece[piece.len() - 1].0);
        let middle = midpoint(a, b);
        if distance(a, b) < 1e-4
            || !(heights.origin.0..high_corner.0).contains(&middle.0)
            || !(heights.origin.1..high_corner.1).contains(&middle.1)
        {
            continue;
        }
        vertical_piece(mesh, a, b, low, high, heights, origin);
    }
}

#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)] // draped curb geometry
fn vertical_piece(
    mesh: &mut MeshData,
    a: Point,
    b: Point,
    low: f64,
    high: f64,
    heights: &HeightGrid,
    origin: [f64; 3],
) {
    let d = direction(a, b);
    let n = [-d.1 as f32, 0.0, -d.0 as f32];
    let base = u32::try_from(mesh.vertices.len()).expect("urban mesh fits u32");
    for (point, lift, u) in [
        (a, low, 0.0),
        (a, high, 0.0),
        (b, high, distance(a, b)),
        (b, low, distance(a, b)),
    ] {
        mesh.vertices.push([
            (point.0 - origin[0]) as f32,
            (heights.at(point.0, point.1) + lift - origin[1]) as f32,
            (-point.1 - origin[2]) as f32,
        ]);
        mesh.normals.push(n);
        mesh.uvs.push([u as f32, lift as f32]);
        mesh.colors.push(Material::Curb.color());
    }
    // Godot's clockwise front faces point against the right-hand cross product.
    mesh.indices
        .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
}
