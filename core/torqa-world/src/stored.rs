//! A built [`World`] as bytes (#237), so a computer can keep the worlds of its courses and ride
//! them without building them again.
//!
//! Little-endian, field after field in the order the types declare them; a list is its length
//! (`u32`) followed by its items. The chunks and the parts built whole go apart, so a keeper can
//! store and read the chunks side by side. Only the same build of Torqa reads what it wrote:
//! worlds are kept by the build that made them, so the format needs no versions of its own and
//! changes with the types.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use crate::{BuildingCell, Horizon, Label, MeshData, Props, TerrainChunk, Trees, World};

/// The first bytes of a world's parts built whole.
const MAGIC: &[u8; 8] = b"TQWORLD1";

/// Writes the parts of `world` built whole, everything but its chunks, to `out`.
///
/// # Errors
/// On write errors, and if a list is longer than `u32::MAX` items.
pub fn write_whole(world: &World, out: &mut impl Write) -> io::Result<()> {
    let World {
        chunks: _,
        road,
        structures,
        railways,
        horizon: Horizon { ground, water },
        fallback_samples,
    } = world;
    let mut out = Writer(out);
    out.0.write_all(MAGIC)?;
    for mesh in [road, structures, railways, ground, water] {
        out.mesh(mesh)?;
    }
    out.len(*fallback_samples)
}

/// Reads what [`write_whole`] wrote: a world without chunks.
///
/// # Errors
/// On read errors, and [`io::ErrorKind::InvalidData`] if the bytes are not a stored world.
pub fn read_whole(input: &mut impl Read) -> io::Result<World> {
    let mut input = Reader(input);
    let mut magic = [0; MAGIC.len()];
    input.0.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid("not a stored world"));
    }
    let world = World {
        chunks: Vec::new(),
        road: input.mesh()?,
        structures: input.mesh()?,
        railways: input.mesh()?,
        horizon: Horizon {
            ground: input.mesh()?,
            water: input.mesh()?,
        },
        fallback_samples: input.len()?,
    };
    input.end()?;
    Ok(world)
}

/// Writes one chunk of a world to `out`.
///
/// # Errors
/// On write errors, and if a list is longer than `u32::MAX` items.
pub fn write_chunk(chunk: &TerrainChunk, out: &mut impl Write) -> io::Result<()> {
    Writer(out).chunk(chunk)
}

/// Reads a chunk [`write_chunk`] wrote.
///
/// # Errors
/// On read errors, and [`io::ErrorKind::InvalidData`] if the bytes are not a stored chunk.
pub fn read_chunk(input: &mut impl Read) -> io::Result<TerrainChunk> {
    let mut input = Reader(input);
    let chunk = input.chunk()?;
    input.end()?;
    Ok(chunk)
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

struct Writer<'a, W: Write>(&'a mut W);

impl<W: Write> Writer<'_, W> {
    fn len(&mut self, len: usize) -> io::Result<()> {
        let len = u32::try_from(len).map_err(|_| invalid("list too long to store"))?;
        self.0.write_all(&len.to_le_bytes())
    }

    fn floats(&mut self, values: &[f32]) -> io::Result<()> {
        self.len(values.len())?;
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.0.write_all(&bytes)
    }

    fn text(&mut self, text: &str) -> io::Result<()> {
        self.len(text.len())?;
        self.0.write_all(text.as_bytes())
    }

    fn models(&mut self, models: &BTreeMap<String, Vec<f32>>) -> io::Result<()> {
        self.len(models.len())?;
        for (name, buffer) in models {
            self.text(name)?;
            self.floats(buffer)?;
        }
        Ok(())
    }

    fn mesh(&mut self, mesh: &MeshData) -> io::Result<()> {
        let MeshData {
            vertices,
            normals,
            uvs,
            colors,
            indices,
        } = mesh;
        self.floats(vertices.as_flattened())?;
        self.floats(normals.as_flattened())?;
        self.floats(uvs.as_flattened())?;
        self.floats(colors.as_flattened())?;
        self.len(indices.len())?;
        let bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        self.0.write_all(&bytes)
    }

    fn chunk(&mut self, chunk: &TerrainChunk) -> io::Result<()> {
        let TerrainChunk {
            center,
            mesh,
            buildings,
            modelled,
            streets,
            tracks,
            water,
            trees:
                Trees {
                    models: plants,
                    grass,
                    flowers,
                },
            props:
                Props {
                    models: props,
                    labels,
                    wires,
                },
        } = chunk;
        self.floats(center)?;
        for mesh in [mesh, buildings] {
            self.mesh(mesh)?;
        }
        self.len(modelled.len())?;
        for BuildingCell { models, shells } in modelled {
            self.models(models)?;
            self.mesh(shells)?;
        }
        for mesh in [streets, tracks, water] {
            self.mesh(mesh)?;
        }
        self.models(plants)?;
        self.floats(grass)?;
        self.floats(flowers)?;
        self.models(props)?;
        self.len(labels.len())?;
        for Label { text, transform } in labels {
            self.text(text)?;
            self.floats(transform)?;
        }
        self.mesh(wires)
    }
}

struct Reader<'a, R: Read>(&'a mut R);

impl<R: Read> Reader<'_, R> {
    /// Fails unless everything was read: bytes left over mean another layout.
    fn end(&mut self) -> io::Result<()> {
        if self.0.read(&mut [0])? == 0 {
            Ok(())
        } else {
            Err(invalid("bytes after the end"))
        }
    }

    fn len(&mut self) -> io::Result<usize> {
        let mut bytes = [0; 4];
        self.0.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes) as usize)
    }

    /// `count` items of four bytes. Read as they come rather than allocated at once, so that a
    /// damaged length ends in an error, not in a request for gigabytes.
    fn words(&mut self, count: usize) -> io::Result<Vec<[u8; 4]>> {
        let mut bytes = Vec::new();
        let wanted = count * 4;
        self.0.take(wanted as u64).read_to_end(&mut bytes)?;
        if bytes.len() != wanted {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(bytes.as_chunks::<4>().0.to_vec())
    }

    fn floats(&mut self) -> io::Result<Vec<f32>> {
        let count = self.len()?;
        Ok(self
            .words(count)?
            .into_iter()
            .map(f32::from_le_bytes)
            .collect())
    }

    /// Floats in groups of `N`, as vertices and transforms are kept.
    fn groups<const N: usize>(&mut self) -> io::Result<Vec<[f32; N]>> {
        let floats = self.floats()?;
        let (groups, rest) = floats.as_chunks::<N>();
        if !rest.is_empty() {
            return Err(invalid("floats do not make whole groups"));
        }
        Ok(groups.to_vec())
    }

    fn one<const N: usize>(&mut self) -> io::Result<[f32; N]> {
        let [group] = self.groups::<N>()?[..] else {
            return Err(invalid("expected one group of floats"));
        };
        Ok(group)
    }

    fn text(&mut self) -> io::Result<String> {
        let len = self.len()?;
        let mut bytes = Vec::new();
        self.0.take(len as u64).read_to_end(&mut bytes)?;
        if bytes.len() != len {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        String::from_utf8(bytes).map_err(|_| invalid("text is not UTF-8"))
    }

    fn models(&mut self) -> io::Result<BTreeMap<String, Vec<f32>>> {
        let count = self.len()?;
        let mut models = BTreeMap::new();
        for _ in 0..count {
            let name = self.text()?;
            models.insert(name, self.floats()?);
        }
        Ok(models)
    }

    fn mesh(&mut self) -> io::Result<MeshData> {
        Ok(MeshData {
            vertices: self.groups()?,
            normals: self.groups()?,
            uvs: self.groups()?,
            colors: self.groups()?,
            indices: {
                let count = self.len()?;
                self.words(count)?
                    .into_iter()
                    .map(u32::from_le_bytes)
                    .collect()
            },
        })
    }

    fn chunk(&mut self) -> io::Result<TerrainChunk> {
        let center = self.one()?;
        let mesh = self.mesh()?;
        let buildings = self.mesh()?;
        let cells = self.len()?;
        let mut modelled = Vec::new();
        for _ in 0..cells {
            modelled.push(BuildingCell {
                models: self.models()?,
                shells: self.mesh()?,
            });
        }
        let streets = self.mesh()?;
        let tracks = self.mesh()?;
        let water = self.mesh()?;
        let trees = Trees {
            models: self.models()?,
            grass: self.floats()?,
            flowers: self.floats()?,
        };
        let models = self.models()?;
        let count = self.len()?;
        let mut labels = Vec::new();
        for _ in 0..count {
            labels.push(Label {
                text: self.text()?,
                transform: self.one()?,
            });
        }
        Ok(TerrainChunk {
            center,
            mesh,
            buildings,
            modelled,
            streets,
            tracks,
            water,
            trees,
            props: Props {
                models,
                labels,
                wires: self.mesh()?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(seed: f32) -> MeshData {
        MeshData {
            vertices: vec![[seed, 1.0, 2.0], [3.0, seed, 5.0]],
            normals: vec![[0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
            uvs: vec![[0.5, seed]],
            colors: vec![[0.1, 0.2, 0.3, 1.0]],
            indices: vec![0, 1, 0, u32::MAX],
        }
    }

    /// A world with every field set apart from its default, so a field the format forgot
    /// comes back different.
    fn sample() -> World {
        let models = |name: &str| BTreeMap::from([(name.to_owned(), vec![1.5, -2.5, f32::MAX])]);
        let chunk = |seed: f32| TerrainChunk {
            center: [seed, 0.0, -seed],
            mesh: mesh(seed),
            buildings: mesh(seed + 1.0),
            modelled: vec![BuildingCell {
                models: models("house"),
                shells: mesh(seed + 2.0),
            }],
            streets: mesh(seed + 3.0),
            tracks: mesh(seed + 4.0),
            water: mesh(seed + 5.0),
            trees: Trees {
                models: models("pine"),
                grass: vec![seed; 12],
                flowers: vec![-seed; 12],
            },
            props: Props {
                models: models("guard_rail"),
                labels: vec![Label {
                    text: "Prêles 12 %".to_owned(),
                    transform: [seed; 12],
                }],
                wires: mesh(seed + 6.0),
            },
        };
        World {
            chunks: vec![chunk(1.0), chunk(20.0)],
            road: mesh(30.0),
            structures: mesh(31.0),
            railways: mesh(32.0),
            horizon: Horizon {
                ground: mesh(33.0),
                water: mesh(34.0),
            },
            fallback_samples: 7,
        }
    }

    fn stored(world: &World) -> (Vec<u8>, Vec<Vec<u8>>) {
        let mut whole = Vec::new();
        write_whole(world, &mut whole).expect("written");
        let chunks = world
            .chunks
            .iter()
            .map(|chunk| {
                let mut bytes = Vec::new();
                write_chunk(chunk, &mut bytes).expect("written");
                bytes
            })
            .collect();
        (whole, chunks)
    }

    fn read_back((whole, chunks): &(Vec<u8>, Vec<Vec<u8>>)) -> World {
        let mut world = read_whole(&mut whole.as_slice()).expect("read");
        for bytes in chunks {
            world
                .chunks
                .push(read_chunk(&mut bytes.as_slice()).expect("read"));
        }
        world
    }

    #[test]
    fn a_stored_world_reads_back_the_same() {
        let world = sample();
        assert_eq!(read_back(&stored(&world)), world);
        let empty = World::default();
        assert_eq!(read_back(&stored(&empty)), empty);
    }

    #[test]
    fn a_cut_off_or_foreign_file_is_refused() {
        let (whole, chunks) = stored(&sample());
        let chunk = &chunks[0];
        for end in [0, 5, MAGIC.len() + 2, whole.len() / 2, whole.len() - 1] {
            assert!(read_whole(&mut &whole[..end]).is_err(), "read {end} bytes");
        }
        for end in [0, 3, chunk.len() / 2, chunk.len() - 1] {
            assert!(read_chunk(&mut &chunk[..end]).is_err(), "read {end} bytes");
        }
        let longer = |bytes: &[u8]| [bytes, &[0]].concat();
        assert!(read_whole(&mut longer(&whole).as_slice()).is_err());
        assert!(read_chunk(&mut longer(chunk).as_slice()).is_err());
        let mut foreign = whole;
        foreign[0] = b'X';
        let error = read_whole(&mut foreign.as_slice()).expect_err("refused");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn a_damaged_length_fails_without_allocating_it() {
        // The chunk's centre claims four billion floats.
        let mut bytes = u32::MAX.to_le_bytes().to_vec();
        bytes.extend([0; 16]);
        assert!(read_chunk(&mut bytes.as_slice()).is_err());
    }
}
