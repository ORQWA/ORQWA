use super::*;
use orqwa_osm::{Road, RoadDetails, RoadKind, RoadSurface};

struct Hillside(f64);

impl ElevationModel for Hillside {
    fn elevation(
        &mut self,
        _lat: f64,
        lon: f64,
    ) -> impl std::future::Future<Output = Result<f64, String>> + Send {
        let east = (lon - 7.0) * METERS_PER_DEGREE * 46f64.to_radians().cos();
        std::future::ready(Ok(500.0 + self.0 * east))
    }
}

fn roads() -> MapData {
    let specs = [
        (
            200.0,
            RoadClass::Major,
            RoadKind::Primary,
            RoadSurface::Paved,
        ),
        (
            260.0,
            RoadClass::Service,
            RoadKind::Service,
            RoadSurface::Unpaved,
        ),
        (
            320.0,
            RoadClass::Track,
            RoadKind::Track,
            RoadSurface::Unpaved,
        ),
        (
            380.0,
            RoadClass::Path,
            RoadKind::Cycleway,
            RoadSurface::Paved,
        ),
        (440.0, RoadClass::Path, RoadKind::Path, RoadSurface::Paved),
    ];
    MapData {
        roads: specs
            .into_iter()
            .map(|(east, class, kind, surface)| Road {
                class,
                details: RoadDetails {
                    kind,
                    surface,
                    ..RoadDetails::default()
                },
                line: vec![at(east, 100.0), at(east, 900.0)],
                structure: None,
            })
            .collect(),
        ..MapData::default()
    }
}

#[tokio::test]
async fn all_formed_ways_have_level_beds_across_slopes_and_chunk_edges() {
    for slope in [0.25, 1.1] {
        let map = roads();
        let world = generate(
            &route_north(&[]).await,
            &mut Hillside(slope),
            &map,
            &mut |_, _| {},
        )
        .await;
        for (east, half, paved) in [
            (200.0, 3.5, true),
            (260.0, 1.75, false),
            (320.0, 1.4, false),
            (380.0, 1.25, true),
        ] {
            for north in [300.0, 478.0, 479.9, 480.1, 482.0, 700.0] {
                let height = |x| {
                    if paved {
                        street_at(&world, x, -north)
                    } else {
                        height_on(&world, |c| &c.tracks, x, -north)
                    }
                    .expect("road surface")
                };
                let (left, right) = (height(east - half + 0.15), height(east + half - 0.15));
                assert!(
                    (left - right).abs() < 0.025,
                    "tilted road at {east}, {north}: {left}, {right}"
                );
                assert!((f64::from(left) - (500.0 + slope * f64::from(east))).abs() < 0.15);
                assert!(height(east) > ground_at(&world, east, -north).unwrap());
            }
        }
        let path = |x| street_at(&world, x, -500.0).expect("small paved path");
        assert!(
            path(440.65) - path(439.35) > slope as f32,
            "small paths retain the hillside"
        );
        let walls: Vec<_> = world
            .chunks
            .iter()
            .flat_map(|c| c.streets.colors.iter().zip(&c.streets.normals))
            .filter(|(c, _)| c[3] == 0.0)
            .collect();
        if slope < 0.5 {
            assert!(
                walls.is_empty(),
                "modest slopes use earthworks without concrete"
            );
        } else {
            assert!(walls.len() > 100, "steep banks need retaining faces");
            assert!(walls.iter().all(|(_, n)| n[1] < 0.53));
        }
        for chunk in &world.chunks {
            assert_valid(&chunk.streets);
            assert_valid(&chunk.tracks);
            assert_faces_follow_normals(&chunk.streets);
        }
    }
}

#[tokio::test]
async fn ground_profiles_are_shared_at_tag_changes_and_branch_endpoints() {
    struct Bump;
    impl ElevationModel for Bump {
        fn elevation(
            &mut self,
            lat: f64,
            _lon: f64,
        ) -> impl std::future::Future<Output = Result<f64, String>> + Send {
            let north = (lat - 46.0) * METERS_PER_DEGREE;
            std::future::ready(Ok(500.0 + 8.0 * (-(north - 480.0).powi(2) / 150.0).exp()))
        }
    }
    let mut map = roads();
    map.roads.truncate(1);
    map.roads[0].line = vec![at(200.0, 100.0), at(200.0, 480.0)];
    let mut continuation = map.roads[0].clone();
    continuation.details.kind = RoadKind::Secondary;
    continuation.line = vec![at(200.0, 480.0), at(200.0, 900.0)];
    let mut branch = continuation.clone();
    branch.line = vec![at(200.0, 480.0), at(300.0, 480.0)];
    map.roads.extend([continuation, branch]);
    let route = route_north(&[]).await;
    let projection = LocalProjection::for_route(&route);
    let streets = streets::lines(&map, &projection, &mut Bump).await;
    let levels = streets::Levels::new(&streets, &appearance::Index::new(&map, &projection));
    for (east, north) in [
        (200.0, 480.0),
        (200.0, 479.99),
        (200.0, 480.01),
        (200.01, 480.0),
    ] {
        let level = levels.nearest(east, north, 0.0).unwrap().height.unwrap();
        assert!((level - 508.0).abs() < 0.02, "junction height {level}");
    }
}

#[tokio::test]
#[ignore = "writes offline earthworks comparison meshes"]
async fn earthworks_render_fixture() {
    let mut scenes = Vec::new();
    for slope in [0.25, 1.1] {
        let world = generate(
            &route_north(&[]).await,
            &mut Hillside(slope),
            &roads(),
            &mut |_, _| {},
        )
        .await;
        let mut meshes = Vec::new();
        for chunk in &world.chunks {
            for (mesh, terrain) in [
                (&chunk.mesh, true),
                (&chunk.streets, false),
                (&chunk.tracks, false),
            ] {
                let indices: Vec<_> = mesh
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter(|face| {
                        face.iter().all(|&i| {
                            let v = mesh.vertices[i as usize];
                            let (x, north) = (v[0] + chunk.center[0], -v[2] - chunk.center[2]);
                            (160.0..=470.0).contains(&x) && (420.0..=560.0).contains(&north)
                        })
                    })
                    .flatten()
                    .copied()
                    .collect();
                if indices.is_empty() {
                    continue;
                }
                meshes.push(serde_json::json!({
                    "vertices": mesh.vertices.iter().map(|v| [v[0] + chunk.center[0], v[1] - 500.0, v[2] + chunk.center[2]]).collect::<Vec<_>>(),
                    "normals": mesh.normals, "uvs": mesh.uvs, "colors": mesh.colors,
                    "indices": indices, "terrain": terrain,
                }));
            }
        }
        scenes.push(serde_json::json!({"slope": slope, "meshes": meshes}));
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../screenshots/earthworks.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec(&scenes).unwrap()).unwrap();
}
