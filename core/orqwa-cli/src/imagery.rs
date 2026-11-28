//! `imagery-scan`: how much street imagery a route has, and what it says (R307, [ADR 0046]).
//!
//! Measured before anything is built from the images: cells searched, requests made, images
//! within the corridor, and the share of the route with an image on each side. With `--facts`,
//! one image per stretch and side is downloaded, read, and its facts written out for review.
//!
//! [ADR 0046]: ../../../docs/adr/0046-street-imagery-mapillary.md

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Args;
use orqwa_domain::files::UsedFiles;
use orqwa_domain::imagery::{Corridor, StreetFacts, StreetImagery};
use orqwa_imagery::{
    Chosen, Located, Mapillary, Prepared, Side, choose, frontage_name, locate, material_name,
    prepare,
};
use serde_json::{Value, json};

/// How far apart the route is sampled for coverage, in metres.
const SAMPLE_STEP: f64 = 10.0;

#[derive(Args)]
pub struct ImageryScanArgs {
    /// GPX file.
    pub file: PathBuf,
    /// How far from the route, in metres, an image may lie.
    #[arg(long, default_value_t = 30.0)]
    pub corridor: f64,
    /// How far along the route an image may be and still count for a sample, in metres.
    #[arg(long, default_value_t = 25.0)]
    pub spacing: f64,
    /// Use only the cache.
    #[arg(long)]
    pub offline: bool,
    /// Ask Mapillary again even where the cache is fresh.
    #[arg(long)]
    pub refresh: bool,
    /// Where to cache, instead of ORQWA's cache directory.
    #[arg(long)]
    pub cache: Option<PathBuf>,
    /// Choose images and derive facts from them; downloads thumbnails the first time.
    #[arg(long)]
    pub facts: bool,
    /// Where to write the derived facts as JSON, for review.
    #[arg(long, requires = "facts")]
    pub facts_out: Option<PathBuf>,
    /// How far apart chosen images stand, in metres.
    #[arg(long, default_value_t = orqwa_imagery::STRETCH)]
    pub stretch: f64,
}

/// Searches Mapillary along a route and reports what it found. The token comes from
/// `MAPILLARY_TOKEN`.
///
/// # Errors
/// If the route cannot be imported, the imagery cannot be searched, or the facts cannot be
/// written.
pub async fn scan(args: &ImageryScanArgs) -> Result<()> {
    let cache = args
        .cache
        .clone()
        .unwrap_or_else(orqwa_app::paths::cache_dir);
    let imported = orqwa_app::import_route(
        &args.file,
        &cache,
        args.offline,
        &UsedFiles::default(),
        &mut |_, _, _| {},
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let route = &imported.route;
    let points: Vec<(f64, f64)> = route.points().iter().map(|p| (p.lat, p.lon)).collect();
    let name = route
        .name()
        .map(str::to_owned)
        .or_else(|| {
            args.file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();

    let mut mapillary =
        Mapillary::new(cache.clone()).with_token(std::env::var("MAPILLARY_TOKEN").ok());
    if args.offline {
        mapillary = mapillary.offline();
    }
    if args.refresh {
        mapillary = mapillary.refreshing();
    }
    let images = search(&mapillary, &points, args.corridor).await?;
    let located = locate(&points, &images);
    let samples = samples(route.points(), SAMPLE_STEP);
    let covered = coverage(&located, &samples, args.spacing);
    let chosen = choose(&located, args.stretch);
    let prepared = if args.facts {
        derive(&mapillary, &chosen, &cache, !args.offline).await
    } else {
        Prepared::default()
    };
    if let Some(path) = &args.facts_out {
        write_facts(path, &name, &mapillary, &prepared).await?;
    }
    report(&Summary {
        name: &name,
        kilometres: route.length().0 / 1000.0,
        corridor: args.corridor,
        cached: mapillary.cached_cells(),
        fetched: mapillary.fetched_cells(),
        requests: mapillary.requests(),
        paging: mapillary.paging(),
        images: images.len(),
        covered,
        samples: samples.len(),
        chosen: chosen.len(),
        prepared: args.facts.then_some(&prepared),
    });
    Ok(())
}

/// Searches the corridor, reporting cells as they are done.
async fn search(
    mapillary: &Mapillary,
    points: &[(f64, f64)],
    width: f64,
) -> Result<Vec<orqwa_domain::imagery::StreetImage>> {
    let mut shown = 0;
    let images = mapillary
        .along(Corridor { points, width }, &mut |done, total| {
            if total > 0 && done != shown {
                shown = done;
                eprint!("\rsearched {done}/{total} cells");
            }
        })
        .await
        .map_err(anyhow::Error::msg)?;
    eprintln!();
    Ok(images)
}

/// Downloads and reads the chosen images, reporting as they are done.
async fn derive(
    mapillary: &Mapillary,
    chosen: &[Chosen<'_>],
    cache: &Path,
    online: bool,
) -> Prepared {
    let mut shown = 0;
    let prepared = prepare(mapillary, chosen, cache, online, &mut |done, total| {
        if total > 0 && done != shown {
            shown = done;
            eprint!("\rread {done}/{total} stretches");
        }
    })
    .await;
    eprintln!();
    prepared
}

/// Writes the derived facts as JSON, one record per chosen image.
async fn write_facts(
    path: &Path,
    route: &str,
    mapillary: &Mapillary,
    prepared: &Prepared,
) -> Result<()> {
    let images: Vec<Value> = prepared
        .derived
        .iter()
        .map(|derived| {
            json!({
                "id": derived.image.id,
                "lat": derived.image.position.0,
                "lon": derived.image.position.1,
                "heading": derived.image.heading,
                "captured_at": derived.image.captured_at,
                "along": derived.along,
                "offset": derived.offset,
                "side": side_name(derived.side),
                "frame_side": derived.frame_side.as_str(),
                "facts": facts_json(derived.facts),
            })
        })
        .collect();
    let document = json!({
        "route": route,
        "attribution": mapillary.attribution(),
        "images": images,
    });
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| anyhow::anyhow!("creating {}: {error}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(&document)?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(|error| anyhow::anyhow!("writing {}: {error}", path.display()))
}

fn facts_json(facts: StreetFacts) -> Value {
    json!({
        "facade": facts.facade.map(|facade| json!({
            "colour": [facade.colour.red, facade.colour.green, facade.colour.blue],
            "storeys": facade.storeys,
            "material": material_name(facade.material),
        })),
        "roof": facts.roof.map(|roof| json!({
            "colour": [roof.colour.red, roof.colour.green, roof.colour.blue],
            "material": material_name(roof.material),
        })),
        "frontage": frontage_name(facts.frontage),
        "kerb": facts.kerb,
        "greenery": facts.greenery,
    })
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
        Side::On => "on",
    }
}

/// What the scan found.
struct Summary<'a> {
    name: &'a str,
    kilometres: f64,
    corridor: f64,
    cached: usize,
    fetched: usize,
    requests: usize,
    paging: usize,
    images: usize,
    /// Samples with an image anywhere, on the left, on the right.
    covered: (usize, usize, usize),
    samples: usize,
    chosen: usize,
    /// Present when facts were derived.
    prepared: Option<&'a Prepared>,
}

/// How much of the route has an image on each side.
fn coverage(located: &[Located], samples: &[f64], spacing: f64) -> (usize, usize, usize) {
    let mut covered = (0_usize, 0_usize, 0_usize);
    for &along in samples {
        let mut left = false;
        let mut right = false;
        for image in located {
            if (image.along - along).abs() > spacing {
                continue;
            }
            match image.side {
                Side::Left => left = true,
                Side::Right => right = true,
                Side::On => {
                    left = true;
                    right = true;
                }
            }
        }
        if left || right {
            covered.0 += 1;
        }
        if left {
            covered.1 += 1;
        }
        if right {
            covered.2 += 1;
        }
    }
    covered
}

/// Prints what the scan found.
#[allow(clippy::cast_precision_loss)] // counts and metres shown to a tenth
fn report(summary: &Summary) {
    let total = summary.samples.max(1);
    println!("Mapillary imagery along {}", summary.name);
    println!("  route        {:.1} km", summary.kilometres);
    println!("  corridor     {:.0} m", summary.corridor);
    println!(
        "  cells        {} ({} cached, {} fetched)",
        summary.cached + summary.fetched,
        summary.cached,
        summary.fetched
    );
    println!(
        "  requests     {} (+{} paging)",
        summary.requests, summary.paging
    );
    println!(
        "  images       {} within {:.0} m",
        summary.images, summary.corridor
    );
    println!(
        "  coverage     {:.0} % (left {:.0} %, right {:.0} %)",
        percent(summary.covered.0, total),
        percent(summary.covered.1, total),
        percent(summary.covered.2, total)
    );
    let per_km = if summary.kilometres > 0.0 {
        format!(
            "{:.1} images, {:.1} requests",
            summary.images as f64 / summary.kilometres,
            summary.requests as f64 / summary.kilometres
        )
    } else {
        "none (route is empty)".to_owned()
    };
    println!("  per km       {per_km}");
    println!("  chosen       {} stretches", summary.chosen);
    if let Some(prepared) = summary.prepared {
        facts_report(prepared);
    }
    println!("  attribution  © Mapillary contributors, CC BY-SA 4.0");
}

/// What the chosen images said, in aggregate.
#[allow(clippy::cast_precision_loss)] // counts shown as a share
fn facts_report(prepared: &Prepared) {
    println!(
        "  facts        {} read ({} cached, {} thumbnails fetched, {} failed)",
        prepared.derived.len(),
        prepared.cached,
        prepared.downloaded,
        prepared.failed.len()
    );
    for (id, error) in prepared.failed.iter().take(5) {
        eprintln!("  image {id}: {error}");
    }
    let mut storeys: Vec<u8> = Vec::new();
    let mut materials: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut frontages: BTreeMap<&'static str, usize> = BTreeMap::new();
    let (mut kerb, mut greenery) = (0_usize, 0_usize);
    for derived in &prepared.derived {
        if let Some(facade) = derived.facts.facade {
            storeys.push(facade.storeys);
            *materials.entry(material_name(facade.material)).or_default() += 1;
        }
        *frontages
            .entry(frontage_name(derived.facts.frontage))
            .or_default() += 1;
        if derived.facts.kerb {
            kerb += 1;
        }
        if derived.facts.greenery {
            greenery += 1;
        }
    }
    if storeys.is_empty() {
        println!(
            "  walls        none of {} stretches",
            prepared.derived.len()
        );
    } else {
        storeys.sort_unstable();
        println!(
            "  walls        {} of {} stretches, {} storeys median ({})",
            storeys.len(),
            prepared.derived.len(),
            storeys[storeys.len() / 2],
            counts(&materials)
        );
    }
    if !frontages.is_empty() {
        println!("  ground       {}", counts(&frontages));
    }
    let images = prepared.derived.len().max(1);
    println!(
        "  street       kerb {:.0} %, greenery {:.0} %",
        percent(kerb, images),
        percent(greenery, images)
    );
}

/// `name count` pairs, most common first.
fn counts(map: &BTreeMap<&'static str, usize>) -> String {
    let mut pairs: Vec<(&str, usize)> = map.iter().map(|(name, count)| (*name, *count)).collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    pairs
        .iter()
        .map(|(name, count)| format!("{name} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Points along the route every `step` metres, by their distance from the start.
fn samples(points: &[orqwa_routes::RoutePoint], step: f64) -> Vec<f64> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut out = vec![first.distance.0];
    let mut next = first.distance.0 + step;
    for point in points {
        while point.distance.0 >= next {
            out.push(next);
            next += step;
        }
    }
    out
}

#[allow(clippy::cast_precision_loss)]
fn percent(part: usize, total: usize) -> f64 {
    part as f64 * 100.0 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use orqwa_domain::imagery::StreetImage;
    use orqwa_domain::units::Meters;
    use orqwa_imagery::FrameSide;
    use orqwa_routes::{RoutePoint, Surface};

    fn point(distance: f64) -> RoutePoint {
        RoutePoint {
            lat: 46.0,
            lon: 7.0,
            elevation: Meters(0.0),
            distance: Meters(distance),
            surface: Surface::Ground,
        }
    }

    #[test]
    fn samples_are_evenly_spaced_between_the_ends() {
        let points: Vec<RoutePoint> = (0..=100).map(|n| point(f64::from(n) * 5.0)).collect();
        let found = samples(&points, 10.0);
        assert_eq!(found.len(), 51);
        assert!((found[0] - 0.0).abs() < f64::EPSILON);
        assert!((found[1] - 10.0).abs() < f64::EPSILON);
        assert!((found[50] - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_route_shorter_than_one_step_has_one_sample() {
        let points = [point(0.0), point(4.0)];
        assert_eq!(samples(&points, 10.0).len(), 1);
    }

    #[test]
    fn coverage_counts_each_side_and_images_near_the_sample() {
        let route = [(46.0, 7.0), (46.001, 7.0)];
        let images = [
            StreetImage {
                id: "left".to_owned(),
                position: (46.0005, 6.9998),
                heading: 0.0,
                captured_at: 0,
                thumbnail: None,
            },
            StreetImage {
                id: "far".to_owned(),
                position: (46.0009, 7.0),
                heading: 0.0,
                captured_at: 0,
                thumbnail: None,
            },
        ];
        let located = locate(&route, &images);
        let covered = coverage(&located, &[0.0, 55.0, 111.0], 25.0);
        // 55 m sees the left image; 111 m sees the one on the line, which counts for both.
        assert_eq!(covered, (2, 2, 1));
    }

    #[test]
    fn facts_are_written_with_their_colour_and_material() {
        let facts = StreetFacts {
            facade: Some(orqwa_domain::imagery::FacadeFacts {
                colour: orqwa_domain::imagery::Rgb {
                    red: 168,
                    green: 84,
                    blue: 60,
                },
                storeys: 3,
                material: orqwa_domain::imagery::Material::Brick,
            }),
            roof: None,
            frontage: orqwa_domain::imagery::Frontage::Wall,
            kerb: true,
            greenery: false,
        };
        let json = facts_json(facts);
        assert_eq!(json["facade"]["material"], "brick");
        assert_eq!(json["facade"]["colour"], json!([168, 84, 60]));
        assert_eq!(json["frontage"], "wall");
        assert_eq!(json["kerb"], true);
    }

    #[test]
    fn counts_are_most_common_first() {
        let mut map = BTreeMap::new();
        map.insert("brick", 2);
        map.insert("plaster", 5);
        assert_eq!(counts(&map), "plaster 5, brick 2");
    }

    #[test]
    fn frame_side_names_are_stable() {
        assert_eq!(FrameSide::Left.as_str(), "left");
        assert_eq!(FrameSide::Both.as_str(), "both");
    }
}
