//! Rider-visible urban surfaces and the green spaces and carriageways they must preserve.

use super::*;

fn city() -> MapData {
    MapData {
        areas: vec![Area {
            cover: LandCover::Residential,
            outer: vec![rectangle(0.0, 500.0, 180.0, 550.0)],
            inner: vec![],
        }],
        ..MapData::default()
    }
}

fn house(id: i64, east: f64, north: f64, half: f64) -> Building {
    Building {
        id,
        outline: square(east, north, half),
        height: None,
        levels: Some(2.0),
        color: None,
    }
}

fn urban_vertices(world: &World) -> impl Iterator<Item = ([f32; 3], [f32; 3], f32)> + '_ {
    world.chunks.iter().flat_map(|chunk| {
        chunk
            .urban
            .vertices
            .iter()
            .zip(&chunk.urban.normals)
            .zip(&chunk.urban.colors)
            .map(move |((v, n), c)| {
                (
                    [v[0] + chunk.center[0], v[1], v[2] + chunk.center[2]],
                    *n,
                    c[0],
                )
            })
    })
}

#[tokio::test]
async fn sidewalks_have_raised_curbs_and_leave_roads_junctions_and_green_spaces_open() {
    let mut map = city();
    map.roads.push(road_north());
    for (cover, east, north) in [
        (LandCover::Meadow, 30.0, 300.0),
        (LandCover::Water, -30.0, 700.0),
    ] {
        map.areas.push(Area {
            cover,
            outer: vec![square(east, north, 25.0)],
            inner: vec![],
        });
    }
    map.roads.push(orqwa_osm::Road {
        class: orqwa_osm::RoadClass::Street,
        line: vec![at(-100.0, 500.0), at(100.0, 500.0)],
        structure: None,
    });
    let world = world(&map).await;
    for chunk in &world.chunks {
        assert_valid(&chunk.urban);
        assert_faces_follow_normals(&chunk.urban);
        for vertex in &chunk.urban.vertices {
            assert!(
                vertex[0].abs() <= CHUNK_SIZE as f32 / 2.0 + 0.01
                    && vertex[2].abs() <= CHUNK_SIZE as f32 / 2.0 + 0.01,
                "urban surface spills beyond its chunk"
            );
        }
    }
    let vertices: Vec<_> = urban_vertices(&world).collect();
    assert!(vertices.len() > 100, "no urban sidewalks");
    let mut curbs = 0;
    for ([x, y, z], normal, material) in vertices {
        assert!(
            x.abs() > 3.0 || !(0.0..=1000.0).contains(&-z),
            "paving on the ridden road"
        );
        assert!(
            x.abs() > 100.0 || (z + 500.0).abs() >= 2.75,
            "paving across a joining street"
        );
        assert!(
            (x - 30.0).abs() >= 25.0 || (z + 300.0).abs() >= 25.0,
            "paving in the park"
        );
        assert!(
            (x + 30.0).abs() >= 25.0 || (z + 700.0).abs() >= 25.0,
            "paving in water"
        );
        if material == 0.75 && x.abs() < 3.3 && normal[1].abs() < 0.1 {
            let ground = ground_at(&world, x, z).expect("terrain below curb");
            let rise = y - ground;
            assert!((0.14..0.30).contains(&rise), "curb rise {rise}");
            assert!(normal[0] * x < 0.0, "curb should face the road");
            curbs += 1;
        }
    }
    assert!(curbs > 10, "no raised curb faces");
}

#[tokio::test]
async fn homes_and_businesses_have_access_and_forecourts_with_open_curbs() {
    let mut map = city();
    map.buildings = vec![house(105, 25.0, 250.0, 6.0), house(106, -25.0, 700.0, 8.0)];
    map.shops.push(at(-25.0, 700.0));
    let world = world(&map).await;
    for chunk in &world.chunks {
        assert_faces_follow_normals(&chunk.urban);
    }
    let paved = |x, z| height_on(&world, |chunk| &chunk.urban, x, z).is_some();
    assert!(paved(12.0, -250.0), "no path between the house and street");
    assert!(paved(18.5, -250.0), "no paved house frontage");
    assert!(paved(-10.0, -700.0), "no business access");
    assert!(paved(-16.0, -700.0), "no shop forecourt");
    assert!(!paved(25.0, -250.0), "paving inside the house");
    let opening =
        height_on(&world, |chunk| &chunk.urban, 4.0, -250.0).expect("access across sidewalk");
    let ground = ground_at(&world, 4.0, -250.0).unwrap();
    assert!(
        (opening - ground - ROAD_SINK as f32).abs() < 0.03,
        "curb blocks the house access"
    );
    assert!(
        urban_vertices(&world).any(|([x, y, z], _, material)| {
            material == 0.75
                && (x - 5.6).abs() < 0.5
                && (z + 250.0).abs() < 4.0
                && y - ground_at(&world, x, z).unwrap_or(y) > 0.6
        }),
        "no low front garden walls"
    );
    // A lawn remains beside the path, while the path itself has no grass poking through it.
    let grass: Vec<_> = world
        .chunks
        .iter()
        .flat_map(|chunk| {
            chunk
                .trees
                .grass
                .as_chunks::<12>()
                .0
                .iter()
                .map(move |t| (t[3] + chunk.center[0], t[11] + chunk.center[2]))
        })
        .collect();
    assert!(
        grass
            .iter()
            .any(|&(x, z)| (10.0..17.0).contains(&x) && (-275.0..-260.0).contains(&z)),
        "garden lost its lawn"
    );
    assert!(
        !grass
            .iter()
            .any(|&(x, z)| (6.0..18.0).contains(&x) && (z + 250.0).abs() < 0.65),
        "grass in the entrance path"
    );
}

#[tokio::test]
async fn dense_blocks_get_paving_but_parks_and_suburban_gardens_stay_green() {
    let mut map = city();
    for (row, north) in [200.0, 240.0, 280.0].into_iter().enumerate() {
        for (side, east) in [-27.0, 27.0].into_iter().enumerate() {
            map.buildings.push(Building {
                id: i64::try_from(row * 2 + side).unwrap(),
                outline: rectangle(east, north, 10.0, 14.0),
                height: None,
                levels: Some(4.0),
                color: None,
            });
        }
    }
    map.buildings.push(house(109, 25.0, 700.0, 6.0));
    map.areas.push(Area {
        cover: LandCover::Meadow,
        outer: vec![square(-8.0, 240.0, 8.0)],
        inner: vec![],
    });
    let world = world(&map).await;
    let urban_color = palette::srgb("ground.urban", 0.0);
    let park_color = landcover::color(Some(LandCover::Meadow));
    let town_color = landcover::color(Some(LandCover::Residential));
    let mut paving = 0;
    let mut park = 0;
    let mut garden = 0;
    for chunk in &world.chunks {
        for (v, color) in chunk.mesh.vertices.iter().zip(&chunk.mesh.colors) {
            let (x, z) = (v[0] + chunk.center[0], v[2] + chunk.center[2]);
            if (z + 240.0).abs() < 50.0 && *color == urban_color {
                paving += 1;
            }
            if (-15.0..-1.0).contains(&x) && (z + 240.0).abs() < 7.0 {
                assert_eq!(*color, park_color, "mapped park covered by dense city");
                park += 1;
            }
            if (20.0..50.0).contains(&x) && (z + 700.0).abs() < 20.0 && *color == town_color {
                garden += 1;
            }
        }
    }
    assert!(
        paving > 0 && park > 0 && garden > 0,
        "paving {paving}, park {park}, garden {garden}"
    );
}

#[tokio::test]
async fn countryside_and_structures_do_not_get_inferred_city_sidewalks() {
    let rural = world(&MapData::default()).await;
    assert_eq!(props_of(&rural, "bench"), [] as [[f64; 3]; 0]);
    assert!(
        rural
            .chunks
            .iter()
            .all(|chunk| chunk.urban.vertices.is_empty())
    );
    let structures = [Structure {
        kind: StructureKind::Bridge,
        line: vec![at(0.0, 300.0), at(0.0, 700.0)],
    }];
    let urban = generate(
        &route_north(&structures).await,
        &mut EastwardSlope,
        &city(),
        &mut |_, _| {},
    )
    .await;
    assert!(
        !urban_vertices(&urban)
            .any(|([x, _, z], _, _)| x.abs() < 6.0 && (-680.0..-320.0).contains(&z)),
        "sidewalk on a bridge"
    );
    assert!(
        !props_of(&urban, "bench")
            .iter()
            .any(|p| (320.0..680.0).contains(&p[2])),
        "bench on a bridge"
    );
}

#[tokio::test]
async fn city_benches_use_free_sidewalks_without_blocking_entrances_or_junctions() {
    let mut map = city();
    map.roads.push(road_north());
    map.buildings = vec![
        house(201, -25.0, 140.0, 6.0),
        house(202, -25.0, 260.0, 6.0),
        house(203, 25.0, 260.0, 6.0),
        Building {
            id: 204,
            outline: rectangle(12.0, 30.0, 5.0, 20.0),
            height: None,
            levels: Some(2.0),
            color: None,
        },
    ];
    map.shops.push(at(12.0, 30.0));
    map.areas.push(Area {
        cover: LandCover::Meadow,
        outer: vec![square(8.0, 500.0, 5.0)],
        inner: vec![],
    });
    map.roads.push(orqwa_osm::Road {
        class: orqwa_osm::RoadClass::Street,
        line: vec![at(-100.0, 620.0), at(100.0, 620.0)],
        structure: None,
    });
    let world = world(&map).await;
    let benches = props_of(&world, "bench");
    let bins = props_of(&world, "bin");
    assert!(benches.len() >= 4, "too little city seating: {benches:?}");
    assert_eq!(benches.len(), bins.len());
    assert!(
        benches
            .iter()
            .any(|p| p[0] > 0.0 && (p[2] - 20.0).abs() < 0.1),
        "a lowered shop frontage can hold a bench away from its entrance"
    );
    let mut along: Vec<_> = benches.iter().map(|p| p[2]).collect();
    along.sort_by(f64::total_cmp);
    assert!(
        along.windows(2).all(|pair| pair[1] - pair[0] > 89.0),
        "duplicate seating across chunks"
    );
    assert!(
        benches
            .iter()
            .any(|p| p[0] > 0.0 && (p[2] - 140.0).abs() < 0.1),
        "try the opposite sidewalk when an entrance blocks the preferred side"
    );
    assert!(
        benches
            .iter()
            .any(|p| p[0] < 0.0 && (p[2] - 500.0).abs() < 0.1),
        "try the opposite sidewalk beside a park"
    );
    assert!(
        benches
            .iter()
            .all(|p| (p[2] - 260.0).abs() > 3.0 && (p[2] - 620.0).abs() > 8.0),
        "leave both entrance paths and the junction clear: {benches:?}"
    );
    for [east, height, north] in benches.iter().chain(&bins) {
        let sidewalk = height_on(&world, |chunk| &chunk.urban, *east as f32, -*north as f32)
            .expect("furniture stands on a drawn sidewalk");
        assert!(
            (height - f64::from(sidewalk)).abs() < 0.01,
            "furniture at {east}, {height}, {north}, sidewalk {sidewalk}"
        );
    }
    for seat in &benches {
        assert!(
            bins.iter().any(|bin| {
                (bin[0] - seat[0]).abs() < 0.01 && (bin[2] - seat[2] - 1.4).abs() < 0.01
            }),
            "no bin beside {seat:?}"
        );
    }
}

#[tokio::test]
async fn steep_city_sidewalks_do_not_get_benches() {
    use std::fmt::Write as _;

    struct Uphill;
    impl ElevationModel for Uphill {
        fn elevation(
            &mut self,
            lat: f64,
            _lon: f64,
        ) -> impl std::future::Future<Output = Result<f64, String>> + Send {
            std::future::ready(Ok(500.0 + (lat - 46.0) * METERS_PER_DEGREE * 0.12))
        }
    }
    let mut xml = String::from("<gpx><trk><trkseg>");
    for i in 0..=100 {
        let north = f64::from(i) * 10.0;
        let (lat, lon) = at(0.0, north);
        let elevation = 500.0 + north * 0.12;
        let _ = write!(
            xml,
            r#"<trkpt lat="{lat}" lon="{lon}"><ele>{elevation}</ele></trkpt>"#
        );
    }
    xml.push_str("</trkseg></trk></gpx>");
    let route = Route::from_gpx(&xml, None).await.unwrap();
    let world = generate(&route, &mut Uphill, &city(), &mut |_, _| {}).await;
    assert!(
        world
            .chunks
            .iter()
            .any(|chunk| !chunk.urban.vertices.is_empty())
    );
    assert_eq!(props_of(&world, "bench"), [] as [[f64; 3]; 0]);
    assert_eq!(props_of(&world, "bin"), [] as [[f64; 3]; 0]);
}
