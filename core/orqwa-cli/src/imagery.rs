//! `imagery-scan`: how much street imagery a route has (R307, [ADR 0046]).
//!
//! Measured before anything is built from the images: cells searched, requests made, images
//! within the corridor, and the share of the route with an image on each side.
//!
//! [ADR 0046]: ../../../docs/adr/0046-street-imagery-mapillary.md

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use orqwa_domain::files::UsedFiles;
use orqwa_domain::imagery::{Corridor, StreetImagery};
use orqwa_imagery::{Located, Mapillary, Side, locate};

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
}

/// Searches Mapillary along a route and reports what it found. The token comes from
/// `MAPILLARY_TOKEN`.
///
/// # Errors
/// If the route cannot be imported or the imagery cannot be searched.
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

    let mut mapillary = Mapillary::new(cache).with_token(std::env::var("MAPILLARY_TOKEN").ok());
    if args.offline {
        mapillary = mapillary.offline();
    }
    if args.refresh {
        mapillary = mapillary.refreshing();
    }
    let mut shown = 0;
    let images = mapillary
        .along(
            Corridor {
                points: &points,
                width: args.corridor,
            },
            &mut |done, total| {
                if total > 0 && done != shown {
                    shown = done;
                    eprint!("\rsearched {done}/{total} cells");
                }
            },
        )
        .await
        .map_err(anyhow::Error::msg)?;
    eprintln!();

    let located = locate(&points, &images);
    let samples = samples(route.points(), SAMPLE_STEP);
    let covered = coverage(&located, &samples, args.spacing);
    let name = route
        .name()
        .map(str::to_owned)
        .or_else(|| {
            args.file
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
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
    });
    Ok(())
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
    println!("  attribution  © Mapillary contributors, CC BY-SA 4.0");
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
            },
            StreetImage {
                id: "far".to_owned(),
                position: (46.0009, 7.0),
                heading: 0.0,
                captured_at: 0,
            },
        ];
        let located = locate(&route, &images);
        let covered = coverage(&located, &[0.0, 55.0, 111.0], 25.0);
        // 55 m sees the left image; 111 m sees the one on the line, which counts for both.
        assert_eq!(covered, (2, 2, 1));
    }
}
