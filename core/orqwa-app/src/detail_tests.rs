//! Courses prepared with map detail (ADR 0033): the surfaces of a region the rider downloaded,
//! carried in the course file.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use orqwa_osm::{LatLon, Osm, Road, RoadKind, RoadSurface};

use crate::tests::{run_until, temp_dir};
use crate::{App, AppEvent, WorldPlan, course};

/// The tile of the test data: Wabern, near the Gurten.
const TILE: (u32, u32) = (8531, 5767);
const EXTRACT: &str = "bern-2026-10-24";

/// A cache with the test tile, and the longest local street in it.
fn cache_with_tile(cache: &Path) -> Road {
    let osm = cache.join("osm");
    let tile = orqwa_osm::tile_path(&osm, 14, TILE);
    std::fs::create_dir_all(tile.parent().unwrap()).unwrap();
    std::fs::write(
        &tile,
        include_bytes!("../../orqwa-osm/tests/data/14_8531_5767.pbf"),
    )
    .unwrap();
    let map = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let osm = Osm::new(osm).offline();
        osm.around(&[(46.9296, 7.4512)], 50.0, &mut |_, _| {})
            .await
            .unwrap()
    });
    let length = |road: &Road| {
        road.line
            .windows(2)
            .map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1))
            .sum::<f64>()
    };
    map.roads
        .into_iter()
        .filter(|r| r.details.kind == RoadKind::Local && r.details.surface != RoadSurface::Unpaved)
        .max_by(|a, b| length(a).total_cmp(&length(b)))
        .unwrap()
}

/// The region Bern made ready in `cache`, its detail cobbling `street`, as a download and its
/// extraction leave them.
fn install_bern(cache: &Path, street: &Road) {
    let osm = cache.join("osm");
    let line: Vec<String> = street
        .line
        .iter()
        .map(|&(lat, lon)| format!("[{},{}]", (lat * 1e7).round(), (lon * 1e7).round()))
        .collect();
    let cell = osm
        .join(orqwa_osm::detail::VERSION)
        .join(EXTRACT)
        .join("12")
        .join((TILE.0 >> 2).to_string())
        .join(format!("{}.json", TILE.1 >> 2));
    std::fs::create_dir_all(cell.parent().unwrap()).unwrap();
    let json = format!(
        r#"{{"surfaces":[{{"id":1,"kind":"local","surface":"cobbles","line":[{}]}}]}}"#,
        line.join(",")
    );
    std::fs::write(cell, json).unwrap();
    let record = osm.join(orqwa_osm::detail::DOWNLOADS).join("bern.json");
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    let json = format!(r#"{{"id":"bern","name":"Bern","extract":"{EXTRACT}"}}"#);
    std::fs::write(record, json).unwrap();
}

/// A GPX file along the street.
fn gpx_along(dir: &Path, street: &Road) -> PathBuf {
    let mut xml = String::from("<gpx><trk><name>Cobbles</name><trkseg>");
    for &(lat, lon) in &street.line {
        let _ = write!(
            xml,
            r#"<trkpt lat="{lat}" lon="{lon}"><ele>550</ele></trkpt>"#
        );
    }
    xml.push_str("</trkseg></trk></gpx>");
    let path = dir.join("cobbles.gpx");
    std::fs::write(&path, xml).unwrap();
    path
}

fn prepare(app: &mut App, gpx: &Path) -> PathBuf {
    app.load_route(gpx.to_owned(), true);
    let events = run_until(app, |e| {
        matches!(e, AppEvent::CourseAdded(_) | AppEvent::Error(_))
    });
    match events.last() {
        Some(AppEvent::CourseAdded(course)) => course.clone(),
        other => panic!("not prepared: {other:?}"),
    }
}

fn route_points(app: &App) -> Vec<LatLon> {
    let route = app.route.as_ref().unwrap();
    route.points().iter().map(|p| (p.lat, p.lon)).collect()
}

/// The surfaces of the course's roads along `street` once it is opened, on another computer
/// with an empty cache.
fn surfaces_on_opening(dir: &Path, course: &Path, street: &Road) -> Vec<RoadSurface> {
    let other = dir.join(format!("other-{}", course.file_stem().unwrap().display()));
    let mut app = App::new(other.join("data"), other.join("cache")).unwrap();
    app.open_course(course.to_owned());
    run_until(&mut app, |e| {
        matches!(e, AppEvent::RouteLoaded(_) | AppEvent::Error(_))
    });
    let WorldPlan::OnRequest(Some(map)) = &app.world_plan else {
        panic!("no map data for the world");
    };
    let surfaces = map
        .roads
        .iter()
        .filter(|r| r.line.iter().all(|p| street.line.contains(p)))
        .map(|r| r.details.surface)
        .collect();
    app.shutdown();
    surfaces
}

#[test]
fn a_course_prepared_with_a_regions_detail_carries_its_surfaces() {
    let dir = temp_dir("detail-course");
    let cache = dir.join("cache");
    let street = cache_with_tile(&cache);
    let gpx = gpx_along(&dir, &street);
    let mut app = App::new(dir.join("data"), cache.clone()).unwrap();
    let plain = prepare(&mut app, &gpx);
    let plain_route = route_points(&app);
    let plain_course = dir.join("plain.tqc");
    std::fs::rename(&plain, &plain_course).unwrap();

    install_bern(&cache, &street);
    let cobbled = prepare(&mut app, &gpx);
    app.shutdown();

    assert_eq!(
        route_points(&app),
        plain_route,
        "the route matched as before"
    );
    let unpacked_to = dir.join("unpacked");
    let manifest = course::unpack(&cobbled, &unpacked_to).unwrap().manifest;
    assert_eq!(manifest.map_detail, [EXTRACT]);
    let cell = format!(
        "osm/detail-v1/{EXTRACT}/12/{}/{}.json",
        TILE.0 >> 2,
        TILE.1 >> 2
    );
    assert!(
        unpacked_to.join(cell).is_file(),
        "the cell it used goes along"
    );
    let surfaces = surfaces_on_opening(&dir, &cobbled, &street);
    assert!(!surfaces.is_empty() && surfaces.iter().all(|&s| s == RoadSurface::Cobbles));

    // Prepared before the region was there, a course stays as it was.
    let manifest = course::unpack(&plain_course, &dir.join("plain"))
        .unwrap()
        .manifest;
    assert_eq!(manifest.map_detail, Vec::<String>::new());
    let surfaces = surfaces_on_opening(&dir, &plain_course, &street);
    assert!(!surfaces.is_empty() && !surfaces.contains(&RoadSurface::Cobbles));
    std::fs::remove_dir_all(dir).unwrap();
}
