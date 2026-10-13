//! The countries a route passes (#230), from the vector tiles alone: which side of the nearest
//! border between countries each stretch of the route lies on. The borders name the countries
//! on their sides by their three-letter codes, and the countries' labels give their names.
//!
//! The tiles leave a side's country out where `OpenMapTiles` could not tell it, often for large
//! countries (Germany, Austria and Poland in 2026). A stretch nearest such a side is in no known
//! country rather than guessed.

use std::collections::{BTreeMap, BTreeSet};

use futures::{StreamExt, stream};
use tracing::info;

use crate::{LatLon, Osm, OsmError, PARALLEL_DOWNLOADS, iso, mvt, tile_of};

/// A country a route passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    /// ISO 3166-1 two-letter code, e.g. `CH`.
    pub code: String,
    /// Its name by language code (`en`, `de`, …), and in its own language(s) under the empty
    /// code; empty if the tiles had no label for it.
    pub names: BTreeMap<String, String>,
}

/// A border between countries, with the three-letter codes of the countries on its left and
/// right (looking along it), where the tiles tell them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Border {
    pub(crate) left: Option<String>,
    pub(crate) right: Option<String>,
    pub(crate) line: Vec<LatLon>,
}

/// Borders and countries' names from tiles.
#[derive(Debug, Default)]
pub(crate) struct CountryData {
    pub(crate) borders: Vec<Border>,
    /// Names by language, by two-letter country code.
    pub(crate) labels: BTreeMap<String, BTreeMap<String, String>>,
}

/// Zoom of the tiles whose borders place most of a route: within about 300 m of where they
/// are, and a few tiles cover a country.
const COARSE_ZOOM: u8 = 6;
/// Zoom of the tiles that place a route near a border: within metres.
const FINE_ZOOM: u8 = 10;
/// Stretches nearer a border than this in the coarse tiles are placed with the fine ones.
const FINE_WITHIN: f64 = 1_000.0;
/// How far around those stretches the fine tiles reach: the border in them may lie a few
/// hundred metres from where the coarse tiles draw it.
const FINE_REACH: f64 = 1_500.0;
/// Zoom of the tile whose labels name the countries: one tile labels every country of Europe.
const NAMES_ZOOM: u8 = 4;
/// Rings of coarse tiles added around a route whose nearest border may lie beyond those
/// loaded.
const MAX_RINGS: u32 = 2;
/// Distance between the places of a route that are looked at.
const SAMPLE_EVERY: f64 = 100.0;
/// A route counts in a country only for this much of it there, so that a road along a border
/// does not count in the country beside it; a route shorter than this counts where most of
/// it is.
const MIN_STRETCH: f64 = 200.0;
const METERS_PER_DEGREE: f64 = 111_195.0;

impl Osm {
    /// The countries the route through `points` passes, in the order it reaches them, from the
    /// borders in the map tiles (downloaded unless offline, and cached). Stretches the tiles
    /// tell no country for count in none.
    ///
    /// # Errors
    /// [`OsmError`] if a tile needed is unavailable: the countries are then not known rather
    /// than guessed from part of the borders.
    pub async fn countries_along(&self, points: &[LatLon]) -> Result<Vec<Country>, OsmError> {
        let Some(&origin) = points.first() else {
            return Ok(Vec::new());
        };
        let flat = Flat::at(origin);
        let samples = samples(points, flat);

        // Coarse borders, from tiles around the route until each place's nearest border is
        // surely among them.
        let (south, north, west, east) = bounds(points);
        let (x0, y0) = tile_of(COARSE_ZOOM, north, west);
        let (x1, y1) = tile_of(COARSE_ZOOM, south, east);
        let mut area = TileArea { x0, y0, x1, y1 };
        let mut coarse = CountryData::default();
        let mut loaded = BTreeSet::new();
        let mut found = Vec::new();
        for ring in 0..=MAX_RINGS {
            let wanted: BTreeSet<_> = area.tiles().difference(&loaded).copied().collect();
            self.load_countries(COARSE_ZOOM, &wanted, &mut coarse)
                .await?;
            loaded.extend(wanted);
            let borders = flatten(&coarse.borders, flat);
            let edges = area.edges(COARSE_ZOOM, flat);
            found = samples.iter().map(|s| nearest(s.at, &borders)).collect();
            let unsure = samples.iter().zip(&found).any(|(s, near)| {
                near.as_ref()
                    .is_none_or(|near| near.distance > edges.inside(s.at))
            });
            if !unsure || ring == MAX_RINGS {
                break;
            }
            area = area.grown(COARSE_ZOOM);
        }

        // Fine borders where the route comes near one.
        let close: Vec<usize> = (0..samples.len())
            .filter(|&i| found[i].as_ref().is_some_and(|n| n.distance < FINE_WITHIN))
            .collect();
        let mut fine = CountryData::default();
        if !close.is_empty() {
            let places: Vec<LatLon> = close.iter().map(|&i| samples[i].position).collect();
            self.load_countries(
                FINE_ZOOM,
                &tiles_near(FINE_ZOOM, &places, FINE_REACH),
                &mut fine,
            )
            .await?;
            let borders = flatten(&fine.borders, flat);
            for i in close {
                if let Some(near) = nearest(samples[i].at, &borders) {
                    found[i] = Some(near);
                }
            }
        }

        let codes = found
            .iter()
            .map(|near| near.as_ref().and_then(|n| n.country.as_deref()));
        let passed = passed(samples.iter().map(|s| s.length).zip(codes));

        // Names from the labels at hand, or from a tile labelling the whole region.
        let mut labels = coarse.labels;
        labels.extend(fine.labels);
        if passed.iter().any(|code| !labels.contains_key(code)) {
            let mut region = CountryData::default();
            let tile = BTreeSet::from([tile_of(NAMES_ZOOM, origin.0, origin.1)]);
            // Names are welcome but not needed: a country stays known by its code.
            if self
                .load_countries(NAMES_ZOOM, &tile, &mut region)
                .await
                .is_ok()
            {
                for (code, names) in region.labels {
                    labels.entry(code).or_insert(names);
                }
            }
        }
        Ok(passed
            .into_iter()
            .map(|code| Country {
                names: labels.get(&code).cloned().unwrap_or_default(),
                code,
            })
            .collect())
    }

    /// Adds the borders and labels of `tiles` at `zoom` to `data`; all of them or an error.
    async fn load_countries(
        &self,
        zoom: u8,
        tiles: &BTreeSet<(u32, u32)>,
        data: &mut CountryData,
    ) -> Result<(), OsmError> {
        if tiles.is_empty() {
            return Ok(());
        }
        info!(zoom, tiles = tiles.len(), "loading borders");
        let template = self.template().await;
        let template = template.as_deref();
        let mut downloads = stream::iter(tiles.iter().copied())
            .map(|tile| async move { (tile, self.tile(zoom, tile, template).await) })
            .buffer_unordered(PARALLEL_DOWNLOADS);
        let mut results = Vec::with_capacity(tiles.len());
        while let Some(result) = downloads.next().await {
            results.push(result);
        }
        results.sort_by_key(|(tile, _)| *tile);
        for (tile, result) in results {
            mvt::merge_countries(zoom, tile, result?, data)?;
        }
        Ok(())
    }
}

/// The two-letter codes of the countries the route passes, in order, from the length of route
/// each place stands for and the three-letter code of its country, if known.
fn passed<'a>(places: impl Iterator<Item = (f64, Option<&'a str>)>) -> Vec<String> {
    let mut lengths: Vec<(String, f64)> = Vec::new();
    for (length, code) in places {
        let Some(code) = code.and_then(iso::alpha_2) else {
            continue;
        };
        match lengths.iter_mut().find(|(c, _)| c.as_str() == code) {
            Some((_, total)) => *total += length,
            None => lengths.push((code.to_owned(), length)),
        }
    }
    let longest = lengths.iter().map(|(_, l)| *l).fold(0.0, f64::max);
    lengths
        .into_iter()
        .filter(|(_, length)| *length >= MIN_STRETCH.min(longest))
        .map(|(code, _)| code)
        .collect()
}

/// A place of the route that is looked at.
struct Sample {
    position: LatLon,
    /// Where it is in [`Flat`] metres.
    at: [f64; 2],
    /// How much of the route it stands for: the way to the next place looked at.
    length: f64,
}

/// Places every [`SAMPLE_EVERY`] metres along `points`, from the first.
fn samples(points: &[LatLon], flat: Flat) -> Vec<Sample> {
    let mut samples: Vec<Sample> = Vec::new();
    let mut since = 0.0;
    let mut previous: Option<[f64; 2]> = None;
    for &position in points {
        let at = flat.project(position);
        if let Some(previous) = previous {
            since += (at[0] - previous[0]).hypot(at[1] - previous[1]);
        }
        previous = Some(at);
        if samples.is_empty() || since >= SAMPLE_EVERY {
            if let Some(last) = samples.last_mut() {
                last.length = since;
            }
            since = 0.0;
            samples.push(Sample {
                position,
                at,
                length: 0.0,
            });
        }
    }
    if let Some(last) = samples.last_mut() {
        last.length = since;
    }
    samples
}

/// South, north, west and east edges of `points`.
fn bounds(points: &[LatLon]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(south, north, west, east), &(lat, lon)| {
            (south.min(lat), north.max(lat), west.min(lon), east.max(lon))
        },
    )
}

/// Tiles at `zoom` within `reach` metres of `places`.
fn tiles_near(zoom: u8, places: &[LatLon], reach: f64) -> BTreeSet<(u32, u32)> {
    let mut tiles = BTreeSet::new();
    for &(lat, lon) in places {
        let d_lat = reach / METERS_PER_DEGREE;
        let d_lon = reach / (METERS_PER_DEGREE * lat.to_radians().cos().max(0.01));
        let (west, north) = tile_of(zoom, lat + d_lat, lon - d_lon);
        let (east, south) = tile_of(zoom, lat - d_lat, lon + d_lon);
        for x in west..=east {
            for y in north..=south {
                tiles.insert((x, y));
            }
        }
    }
    tiles
}

/// Flat metres east and north of a place near the route, for distances and sides.
#[derive(Debug, Clone, Copy)]
struct Flat {
    lat: f64,
    lon: f64,
    east_per_degree: f64,
}

impl Flat {
    fn at((lat, lon): LatLon) -> Self {
        Self {
            lat,
            lon,
            east_per_degree: METERS_PER_DEGREE * lat.to_radians().cos(),
        }
    }

    fn project(self, (lat, lon): LatLon) -> [f64; 2] {
        [
            (lon - self.lon) * self.east_per_degree,
            (lat - self.lat) * METERS_PER_DEGREE,
        ]
    }
}

/// A block of tiles at one zoom.
#[derive(Debug, Clone, Copy)]
struct TileArea {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl TileArea {
    fn tiles(self) -> BTreeSet<(u32, u32)> {
        (self.x0..=self.x1)
            .flat_map(|x| (self.y0..=self.y1).map(move |y| (x, y)))
            .collect()
    }

    /// One tile more on every side, within the map.
    fn grown(self, zoom: u8) -> Self {
        let last = (1u32 << zoom) - 1;
        Self {
            x0: self.x0.saturating_sub(1),
            y0: self.y0.saturating_sub(1),
            x1: (self.x1 + 1).min(last),
            y1: (self.y1 + 1).min(last),
        }
    }

    /// Its edges in `flat` metres: west, east, south, north.
    fn edges(self, zoom: u8, flat: Flat) -> Edges {
        let corner = |x: u32, y: u32| {
            let n = f64::from(1u32 << zoom);
            let lon = f64::from(x) / n * 360.0 - 180.0;
            let lat = (std::f64::consts::PI * (1.0 - 2.0 * f64::from(y) / n))
                .sinh()
                .atan()
                .to_degrees();
            flat.project((lat, lon))
        };
        let north_west = corner(self.x0, self.y0);
        let south_east = corner(self.x1 + 1, self.y1 + 1);
        Edges {
            west: north_west[0],
            east: south_east[0],
            south: south_east[1],
            north: north_west[1],
        }
    }
}

/// The edges of the land whose borders are loaded.
struct Edges {
    west: f64,
    east: f64,
    south: f64,
    north: f64,
}

impl Edges {
    /// How far `at` lies inside the edges: a border nearer than this is surely the nearest.
    fn inside(&self, at: [f64; 2]) -> f64 {
        (at[0] - self.west)
            .min(self.east - at[0])
            .min(at[1] - self.south)
            .min(self.north - at[1])
    }
}

/// A border in [`Flat`] metres, with its bounding box for skipping far ones quickly.
struct FlatBorder<'a> {
    left: Option<&'a str>,
    right: Option<&'a str>,
    points: Vec<[f64; 2]>,
    min: [f64; 2],
    max: [f64; 2],
}

impl<'a> FlatBorder<'a> {
    fn new(left: Option<&'a str>, right: Option<&'a str>, points: Vec<[f64; 2]>) -> Self {
        let (min, max) = points
            .iter()
            .fold(([f64::MAX; 2], [f64::MIN; 2]), |(min, max), p| {
                (
                    [min[0].min(p[0]), min[1].min(p[1])],
                    [max[0].max(p[0]), max[1].max(p[1])],
                )
            });
        Self {
            left,
            right,
            points,
            min,
            max,
        }
    }
}

fn flatten(borders: &[Border], flat: Flat) -> Vec<FlatBorder<'_>> {
    borders
        .iter()
        .filter(|b| b.line.len() >= 2)
        .map(|border| {
            FlatBorder::new(
                border.left.as_deref(),
                border.right.as_deref(),
                border.line.iter().map(|&p| flat.project(p)).collect(),
            )
        })
        .collect()
}

/// The nearest border to a place: how far it is, and the three-letter code of the country on
/// the place's side of it, if the tiles tell it.
#[derive(Debug, Clone, PartialEq)]
struct Nearest {
    distance: f64,
    country: Option<String>,
}

fn nearest(at: [f64; 2], borders: &[FlatBorder<'_>]) -> Option<Nearest> {
    // (distance, border, segment, position along it)
    let mut best: Option<(f64, usize, usize, f64)> = None;
    for (index, border) in borders.iter().enumerate() {
        let dx = (border.min[0] - at[0]).max(at[0] - border.max[0]).max(0.0);
        let dy = (border.min[1] - at[1]).max(at[1] - border.max[1]).max(0.0);
        if best.is_some_and(|(d, ..)| dx.hypot(dy) >= d) {
            continue;
        }
        for (segment, pair) in border.points.windows(2).enumerate() {
            let (distance, t) = to_segment(at, pair[0], pair[1]);
            if best.is_none_or(|(d, ..)| distance < d) {
                best = Some((distance, index, segment, t));
            }
        }
    }
    let (distance, index, segment, t) = best?;
    let border = &borders[index];
    let country = if on_left(&border.points, segment, t, at) {
        border.left
    } else {
        border.right
    };
    Some(Nearest {
        distance,
        country: country.map(ToOwned::to_owned),
    })
}

/// Distance from `p` to segment `a`–`b`, and where along it (0–1) the nearest point lies.
fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy), t)
}

/// Positive where `p` lies left of the way from `a` to `b`.
fn cross(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Whether `p` lies left of the line `points`, whose nearest point to it is at `t` along
/// `segment`.
fn on_left(points: &[[f64; 2]], segment: usize, t: f64, p: [f64; 2]) -> bool {
    let corner = if t <= 0.0 {
        Some(segment)
    } else if t >= 1.0 {
        Some(segment + 1)
    } else {
        None
    };
    match corner {
        // Nearest a corner between two segments: where they disagree, `p` lies off the
        // corner's outside, the side away from the turn.
        Some(v) if v > 0 && v + 1 < points.len() => {
            let before = cross(points[v - 1], points[v], p);
            let after = cross(points[v], points[v + 1], p);
            if (before > 0.0) == (after > 0.0) {
                before > 0.0
            } else {
                cross(points[v - 1], points[v], points[v + 1]) < 0.0
            }
        }
        _ => cross(points[segment], points[segment + 1], p) > 0.0,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::tile_path;

    /// A cache holding the fixture tiles of `core/fixtures/countries`, read offline.
    fn fixture_osm(name: &str) -> Osm {
        let cache =
            std::env::temp_dir().join(format!("torqa-osm-countries-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache);
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/countries");
        for (zoom, x, y) in [(6, 33, 22), (10, 533, 357), (10, 533, 358), (4, 8, 5)] {
            let from: PathBuf = fixtures.join(format!("{zoom}/{x}/{y}.pbf"));
            let to = tile_path(&cache, zoom, (x, y));
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(from, to).unwrap();
        }
        Osm::new(cache).offline()
    }

    /// Points every 10 m on straight lines between `places`.
    fn route(places: &[LatLon]) -> Vec<LatLon> {
        let mut points = Vec::new();
        for pair in places.windows(2) {
            let ((lat0, lon0), (lat1, lon1)) = (pair[0], pair[1]);
            let flat = Flat::at(pair[0]);
            let [east, north] = flat.project(pair[1]);
            let length = east.hypot(north);
            let mut along = 0.0;
            while along < length {
                let share = along / length;
                points.push((lat0 + (lat1 - lat0) * share, lon0 + (lon1 - lon0) * share));
                along += 10.0;
            }
        }
        points.extend(places.last());
        points
    }

    #[tokio::test]
    async fn a_route_from_france_through_basel_passes_both_in_order() {
        let osm = fixture_osm("basel");
        // Saint-Louis (France), Basel and Riehen (Switzerland), then over the border to Weil
        // am Rhein (Germany), whose side of the border the tiles leave out.
        let points = route(&[
            (47.5900, 7.5600),
            (47.5596, 7.5886),
            (47.5788, 7.6468),
            (47.5950, 7.6200),
        ]);

        let countries = osm.countries_along(&points).await.unwrap();

        let codes: Vec<&str> = countries.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["FR", "CH"]);
        let switzerland = &countries[1];
        assert_eq!(switzerland.names["en"], "Switzerland");
        assert_eq!(switzerland.names["de"], "Schweiz");
        assert_eq!(countries[0].names["fr"], "France");
    }

    #[tokio::test]
    async fn a_route_well_inside_a_country_needs_no_detailed_tiles() {
        let osm = fixture_osm("bern");
        // Through Bern, tens of kilometres from any border: the coarse tile alone.
        let points = route(&[(46.948, 7.447), (46.960, 7.480)]);

        let countries = osm.countries_along(&points).await.unwrap();

        assert_eq!(countries.len(), 1);
        assert_eq!(countries[0].code, "CH");
    }

    #[tokio::test]
    async fn without_the_tiles_the_countries_are_not_known() {
        let cache =
            std::env::temp_dir().join(format!("torqa-osm-countries-none-{}", std::process::id()));
        let osm = Osm::new(cache).offline();

        let result = osm.countries_along(&[(46.948, 7.447), (46.95, 7.45)]).await;

        assert!(
            matches!(result, Err(OsmError::Unavailable(_))),
            "{result:?}"
        );
    }

    fn border(
        left: Option<&'static str>,
        right: Option<&'static str>,
        line: &[[f64; 2]],
    ) -> FlatBorder<'static> {
        FlatBorder::new(left, right, line.to_vec())
    }

    fn country(at: [f64; 2], borders: &[FlatBorder<'_>]) -> Option<String> {
        nearest(at, borders).and_then(|n| n.country)
    }

    #[test]
    fn a_place_is_in_the_country_on_its_side_of_the_nearest_border() {
        // Due north, FRA on the left (west), CHE on the right (east).
        let borders = [border(
            Some("FRA"),
            Some("CHE"),
            &[[0.0, -1_000.0], [0.0, 1_000.0]],
        )];

        assert_eq!(country([-50.0, 0.0], &borders).as_deref(), Some("FRA"));
        assert_eq!(country([50.0, 0.0], &borders).as_deref(), Some("CHE"));
        assert_eq!(nearest([50.0, 0.0], &borders).unwrap().distance, 50.0);
    }

    #[test]
    fn off_a_corner_the_side_is_the_one_the_border_turns_away_from() {
        // East, then sharply back to the west-north-west: a left turn, its inside on the left.
        let borders = [border(
            Some("AAA"),
            Some("BBB"),
            &[[-1_000.0, 0.0], [0.0, 0.0], [-1_000.0, 100.0]],
        )];

        // Off the corner's outside the place is left of the first segment's line but right
        // of the second's.
        assert_eq!(country([10.0, 5.0], &borders).as_deref(), Some("BBB"));
        assert_eq!(country([10.0, -5.0], &borders).as_deref(), Some("BBB"));
        // Inside the turn.
        assert_eq!(country([-500.0, 20.0], &borders).as_deref(), Some("AAA"));
    }

    #[test]
    fn a_side_the_tiles_leave_out_is_in_no_known_country() {
        let borders = [
            border(None, Some("CHE"), &[[0.0, -1_000.0], [0.0, 1_000.0]]),
            // Farther away and with its side known, but beyond the first border.
            border(
                Some("FRA"),
                Some("ITA"),
                &[[-500.0, -1_000.0], [-500.0, 1_000.0]],
            ),
        ];

        assert_eq!(country([-100.0, 0.0], &borders), None);
        assert_eq!(country([100.0, 0.0], &borders).as_deref(), Some("CHE"));
    }

    #[test]
    fn a_route_counts_where_it_rides_a_while_in_the_order_it_gets_there() {
        let route = [
            (100.0, Some("FRA")),
            (100.0, Some("FRA")),
            (100.0, Some("CHE")),
            (100.0, Some("CHE")),
            // A short dip across a border it runs along.
            (100.0, Some("DEU")),
            (100.0, None),
            (100.0, Some("CHE")),
        ];

        assert_eq!(passed(route.into_iter()), ["FR", "CH"]);
        // Shorter than the minimum: where most of it is.
        let short = [(60.0, Some("FRA")), (90.0, Some("CHE"))];
        assert_eq!(passed(short.into_iter()), ["CH"]);
        assert_eq!(passed([(500.0, None)].into_iter()), Vec::<String>::new());
    }

    #[test]
    fn places_are_looked_at_every_hundred_metres_and_stand_for_their_stretch() {
        let flat = Flat::at((46.0, 7.0));
        // 1 km due north in 10 m steps.
        let points: Vec<LatLon> = (0..=100)
            .map(|i| (46.0 + f64::from(i) * 10.0 / METERS_PER_DEGREE, 7.0))
            .collect();

        let looked_at = samples(&points, flat);

        assert!((10..=11).contains(&looked_at.len()), "{}", looked_at.len());
        let total: f64 = looked_at.iter().map(|s| s.length).sum();
        assert!((total - 1_000.0).abs() < 1e-6, "{total}");
        let lone = samples(&points[..3], flat);
        assert_eq!(lone.len(), 1);
        assert!((lone[0].length - 20.0).abs() < 1e-6);
    }
}
