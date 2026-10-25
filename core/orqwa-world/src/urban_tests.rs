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
        details: orqwa_osm::RoadDetails::default(),
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
        if material == 0.75 && x.abs() < 3.3 && normal[0].abs() > 0.9 {
            let ground = ground_at(&world, x, z).expect("terrain below curb");
            let rise = y - ground;
            assert!((-0.03..0.30).contains(&rise), "curb rise {rise}");
            assert!(normal[0] * x < 0.0, "curb should face the road");
            curbs += 1;
        }
    }
    assert!(curbs > 10, "no raised curb faces");
}

#[tokio::test]
async fn sidewalks_are_grounded_and_join_lowered_access_without_steps() {
    let mut map = city();
    map.buildings = vec![house(301, 25.0, 250.0, 6.0), house(302, 25.0, 480.0, 6.0)];
    let world = world(&map).await;
    let mut sides = 0;
    for chunk in &world.chunks {
        assert_valid(&chunk.urban);
        assert_faces_follow_normals(&chunk.urban);
        for triangle in chunk.urban.indices.as_chunks::<3>().0 {
            if chunk.urban.normals[triangle[0] as usize][1].abs() > 0.01 {
                continue;
            }
            let rises = [triangle[0], triangle[1], triangle[2]].map(|index| {
                let v = chunk.urban.vertices[index as usize];
                v[1] - ground_at(&world, v[0] + chunk.center[0], v[2] + chunk.center[2]).unwrap()
            });
            if rises.iter().copied().fold(f32::MIN, f32::max) > 0.4 {
                continue; // Garden walls have their own ground contact.
            }
            assert!(
                rises.iter().any(|rise| *rise <= 0.001),
                "floating sidewalk face {rises:?}"
            );
            sides += 1;
        }
    }
    assert!(sides > 100, "no solid sidewalk edges");
    for entrance in [250.0, 480.0] {
        let mut previous: Option<f32> = None;
        for step in -100..=100 {
            let z = -(entrance + step as f32 * 0.1);
            let y = height_on(&world, |chunk| &chunk.urban, 4.0, z).expect("continuous sidewalk");
            if let Some(before) = previous {
                assert!(
                    (y - before).abs() < 0.025,
                    "step beside entrance at {z}: {before} to {y}"
                );
            }
            previous = Some(y);
        }
    }
}

#[tokio::test]
async fn sidewalks_join_on_both_sides_of_a_bend() {
    let mut map = city();
    map.roads.push(orqwa_osm::Road {
        class: orqwa_osm::RoadClass::Street,
        details: orqwa_osm::RoadDetails::default(),
        line: vec![at(50.0, 150.0), at(50.0, 200.0), at(80.0, 250.0)],
        structure: None,
    });
    let world = world(&map).await;
    let forward = (30.0_f32 / 3400.0_f32.sqrt(), 50.0_f32 / 3400.0_f32.sqrt());
    let divisor = 1.0 + forward.1;
    for side in [-1.0, 1.0] {
        for width in [3.0, 3.5, 4.0, 4.2] {
            // Cross the joint inside the rounded walking strip, including the
            // wedge two independently offset rectangular slabs leave uncovered.
            let east = 50.0 + width * side;
            let north = 200.0 - forward.0 / divisor * width * side;
            for step in -10..=10 {
                let z = -(north + step as f32 * 0.01);
                assert!(
                    height_on(&world, |chunk| &chunk.urban, east, z).is_some(),
                    "sidewalk gap at {east}, {}",
                    -z
                );
            }
        }
    }
}

#[tokio::test]
async fn sidewalks_follow_rounded_junction_corners_without_square_gaps() {
    let mut map = city();
    map.roads = vec![
        road_north(),
        orqwa_osm::Road {
            class: orqwa_osm::RoadClass::Street,
            details: orqwa_osm::RoadDetails::default(),
            line: vec![at(-100.0, 500.0), at(0.0, 500.0)],
            structure: None,
        },
    ];
    let world = world(&map).await;
    for chunk in &world.chunks {
        assert_valid(&chunk.urban);
        assert_faces_follow_normals(&chunk.urban);
        assert!(
            chunk
                .urban
                .normals
                .iter()
                .all(|normal| { (normal.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 0.001 }),
            "urban lighting normals must be unit vectors"
        );
    }
    // The footway follows the same tangent circle as the road's rounded corner,
    // on both sides of the side street, at its full walking width.
    let radius = (3.0_f32 + 2.75) * 0.6;
    for side in [-1.0, 1.0] {
        let center = (-3.0 - radius, 500.0 + side * (2.75 + radius));
        for step in 0..=32 {
            let angle = step as f32 / 32.0 * std::f32::consts::FRAC_PI_2;
            for across in [0.2, 0.9, 1.6] {
                let r = radius - 0.12 - across;
                let (east, north) = (
                    center.0 + r * angle.cos(),
                    center.1 - side * r * angle.sin(),
                );
                assert!(
                    height_on(&world, |chunk| &chunk.urban, east, -north).is_some(),
                    "gap around junction curb at {east}, {north}"
                );
                assert!(
                    street_at(&world, east, -north).is_none(),
                    "sidewalk in junction asphalt"
                );
            }
        }
    }
    for east in [-2.0, -1.0, 0.0] {
        assert!(
            height_on(&world, |chunk| &chunk.urban, east, -500.0).is_none(),
            "curb across the junction mouth"
        );
    }
}

#[tokio::test]
async fn acute_forks_have_a_rounded_sidewalk_nose_instead_of_cut_off_slabs() {
    let mut map = city();
    map.roads = vec![
        road_north(),
        orqwa_osm::Road {
            class: orqwa_osm::RoadClass::Street,
            details: orqwa_osm::RoadDetails::default(),
            line: vec![at(-40.0, 400.0), at(0.0, 500.0)],
            structure: None,
        },
    ];
    let world = world(&map).await;
    let angle = 0.4_f32.atan();
    let radius = (3.0 + 2.75) * 0.6;
    let apex_north = 500.0 - (2.75 + 3.0 * angle.cos()) / angle.sin();
    let center = (-3.0 - radius, apex_north - radius / (angle / 2.0).tan());
    for step in 1..32 {
        let arc = (std::f32::consts::PI - angle) * step as f32 / 32.0;
        let r = radius - 0.12 - 0.9;
        let (east, north) = (center.0 + r * arc.cos(), center.1 + r * arc.sin());
        assert!(
            height_on(&world, |chunk| &chunk.urban, east, -north).is_some(),
            "gap around fork nose at {east}, {north}"
        );
        assert!(
            street_at(&world, east, -north).is_none(),
            "footway overlaps fork asphalt"
        );
    }
}

#[tokio::test]
async fn outer_sidewalk_bends_are_round_and_curbstones_have_their_own_beveled_border() {
    let mut map = city();
    map.roads.push(orqwa_osm::Road {
        class: orqwa_osm::RoadClass::Street,
        details: orqwa_osm::RoadDetails::default(),
        line: vec![at(50.0, 150.0), at(50.0, 200.0), at(100.0, 200.0)],
        structure: None,
    });
    let world = world(&map).await;
    for step in 1..32 {
        let angle = step as f32 / 32.0 * std::f32::consts::FRAC_PI_2;
        let radius = 2.75 + 0.12 + 0.9;
        let (east, north) = (50.0 - radius * angle.cos(), 200.0 + radius * angle.sin());
        assert!(
            height_on(&world, |chunk| &chunk.urban, east, -north).is_some(),
            "gap around outer bend at {east}, {north}"
        );
    }
    let mut curb_tops = 0;
    for ([x, _, _], normal, material) in urban_vertices(&world) {
        if material == 0.75 && normal[1] > 0.9 && x.abs() < 10.0 {
            assert!(
                (3.12 - 0.001..=3.30 + 0.001).contains(&x.abs()),
                "curbstone outside its narrow border: {x}"
            );
            curb_tops += 1;
        }
    }
    assert!(curb_tops > 100, "no separate curbstone tops");
    let walk = height_on(&world, |chunk| &chunk.urban, 4.0, -250.0).unwrap();
    let lip = height_on(&world, |chunk| &chunk.urban, 3.125, -250.0).unwrap();
    assert!(
        (0.01..0.025).contains(&(walk - lip)),
        "curb lip has no bevel"
    );
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
async fn dense_city_banks_do_not_become_paved_squares() {
    struct Hillside;
    impl ElevationModel for Hillside {
        fn elevation(
            &mut self,
            _lat: f64,
            lon: f64,
        ) -> impl std::future::Future<Output = Result<f64, String>> + Send {
            let east = (lon - 7.0) * METERS_PER_DEGREE * 46_f64.to_radians().cos();
            std::future::ready(Ok(500.0 + east * 0.6))
        }
    }
    let mut map = city();
    for (i, north) in [200.0, 240.0, 280.0].into_iter().enumerate() {
        map.buildings
            .push(house(i64::try_from(i).unwrap(), 27.0, north, 14.0));
    }
    let world = generate(&route_north(&[]).await, &mut Hillside, &map, &mut |_, _| {}).await;
    let urban = palette::srgb("ground.urban", 0.0);
    let mut bank = 0;
    let mut paved = 0;
    for chunk in &world.chunks {
        for ((vertex, normal), color) in chunk
            .mesh
            .vertices
            .iter()
            .zip(&chunk.mesh.normals)
            .zip(&chunk.mesh.colors)
        {
            let (east, north) = (vertex[0] + chunk.center[0], -vertex[2] - chunk.center[2]);
            if (5.0..45.0).contains(&east) && (210.0..270.0).contains(&north) {
                if normal[1] < 0.95 {
                    assert_ne!(*color, urban, "paving on an unwalkable bank");
                    bank += 1;
                }
                if *color == urban {
                    paved += 1;
                }
            }
        }
    }
    assert!(
        bank > 0 && paved > 0,
        "need both a natural bank and level city paving: {bank}, {paved}"
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
        details: orqwa_osm::RoadDetails::default(),
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
