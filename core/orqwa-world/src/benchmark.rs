//! A small, deterministic offline graphics workload (R150). Uses the ride's mesh and
//! instance contracts, models and materials, without importing a course or fetching data.

// Fixture grid coordinates are small, non-negative where converted to palette indices.
#![allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]

use crate::{BuildingCell, MeshData, TerrainChunk, Trees, World, palette};

/// Builds the bundled benchmark: a road between a village, grass, water and dense woods.
/// This is a calibration workload, independent of the course generator and stored worlds.
#[must_use]
pub fn benchmark() -> World {
    let mut world = World::default();
    let mut chunk = TerrainChunk {
        center: [0.0; 3],
        mesh: MeshData::default(),
        buildings: MeshData::default(),
        modelled: Vec::new(),
        streets: MeshData::default(),
        tracks: MeshData::default(),
        urban: MeshData::default(),
        water: MeshData::default(),
        trees: Trees::default(),
        props: crate::Props::default(),
    };
    let meadow = palette::srgb("ground.meadow", 1.0);
    for z in -120..20 {
        quad(
            &mut world.road,
            [-3.5, 0.0, z as f32 * 10.0],
            [7.0, 10.0],
            [0.0, 7.0 / 32.0, 0.25, 0.25],
            true,
        );
    }
    // The lake is a shallow recess, with no ground below the reflective surface.
    for x in -30..30 {
        for z in -80..20 {
            let x = x as f32 * 16.0;
            let z = z as f32 * 16.0;
            let lake = (16.0..112.0).contains(&x) && (-160.0..0.0).contains(&z);
            quad(
                &mut chunk.mesh,
                [x, if lake { -2.0 } else { -0.04 }, z],
                [16.0, 16.0],
                meadow,
                false,
            );
            if lake {
                quad(
                    &mut chunk.water,
                    [x, -0.1, z],
                    [16.0, 16.0],
                    [0.0, 0.0, 0.0, 1.0],
                    false,
                );
            }
        }
    }
    for x in 0..54 {
        for z in 0..200 {
            let east = 20.0 + x as f32 * 7.0;
            let south = 80.0 - z as f32 * 6.0;
            if east < 120.0 && (-170.0..10.0).contains(&south) {
                continue;
            }
            let name = if (x + z) % 2 == 0 {
                "conifer_tall"
            } else {
                "broadleaf_round"
            };
            let buffer = chunk.trees.models.entry(name.to_owned()).or_default();
            let scale = 0.8 + ((x * 7 + z * 3) % 9) as f32 * 0.05;
            transform(buffer, east, south, scale);
            buffer.extend(palette::pick("plants.conifers", (x + z) as usize, 1.0));
        }
    }
    for z in 0..45 {
        let mut buildings = BuildingCell::default();
        for x in 0..8 {
            let buffer = buildings
                .models
                .entry("house_gable_2_m".to_owned())
                .or_default();
            transform(buffer, -22.0 - x as f32 * 24.0, 90.0 - z as f32 * 25.0, 1.0);
            buffer.extend(palette::pick(
                "buildings.light_walls",
                (x + z) as usize,
                1.0,
            ));
            buffer.extend(palette::pick("buildings.tiles", (x + z) as usize, 0.5));
        }
        chunk.modelled.push(buildings);
    }
    // Grass uses the same 1.1 m spacing and 30 m roadside reach as ordinary courses.
    for x in -27_i32..27 {
        for z in -250..100 {
            let east = x as f32 * 1.1;
            let south = z as f32 * 1.1;
            if east.abs() < 4.0 || east > 16.0 && (-160.0..0.0).contains(&south) {
                continue;
            }
            transform(&mut chunk.trees.grass, east, south, 1.0);
        }
    }
    world.chunks.push(chunk);
    world
}

#[allow(clippy::cast_possible_truncation)] // controlled fixture sizes, comfortably below u32
fn quad(mesh: &mut MeshData, at: [f32; 3], size: [f32; 2], color: [f32; 4], road: bool) {
    let [x, y, z] = at;
    let [width, length] = size;
    let first = mesh.vertices.len() as u32;
    mesh.vertices.extend([
        [x, y, z],
        [x + width, y, z],
        [x + width, y, z + length],
        [x, y, z + length],
    ]);
    mesh.normals.extend([[0.0, 1.0, 0.0]; 4]);
    mesh.colors.extend([color; 4]);
    mesh.uvs.extend(if road {
        [[0.0, z], [1.0, z], [1.0, z + length], [0.0, z + length]]
    } else {
        [
            [x, z],
            [x + width, z],
            [x + width, z + length],
            [x, z + length],
        ]
    });
    mesh.indices
        .extend([first, first + 1, first + 2, first, first + 2, first + 3]);
}

fn transform(buffer: &mut Vec<f32>, x: f32, z: f32, scale: f32) {
    buffer.extend([scale, 0.0, 0.0, x, 0.0, scale, 0.0, 0.0, 0.0, 0.0, scale, z]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_workload_has_the_costs_the_presets_control() {
        let world = benchmark();
        let chunk = &world.chunks[0];
        assert_ne!(world.road.vertices.len(), 0);
        assert_ne!(chunk.mesh.vertices.len(), 0);
        assert_ne!(chunk.water.vertices.len(), 0);
        assert_ne!(chunk.trees.grass.len(), 0);
        assert!(!chunk.modelled[0].models.is_empty());
        assert!(chunk.trees.len() > 10_000);
        let low: usize = chunk
            .trees
            .models_for_density(0.125)
            .map(|(_, buffer)| buffer.len())
            .sum();
        let ultra: usize = chunk
            .trees
            .models_for_density(1.0)
            .map(|(_, buffer)| buffer.len())
            .sum();
        assert!(
            low < ultra / 4,
            "the scene must exercise density before upload"
        );
        for mesh in [&world.road, &chunk.mesh, &chunk.water] {
            assert_eq!(mesh.vertices.len(), mesh.colors.len());
            assert_eq!(mesh.vertices.len(), mesh.normals.len());
            assert_eq!(mesh.vertices.len(), mesh.uvs.len());
        }
    }
}
