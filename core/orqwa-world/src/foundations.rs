//! Foundations meet the final terrain triangles, after shaping and channel carving. Raw DEM
//! samples and interpolated helper heights can stand well above the ground actually drawn.

use crate::{CHUNK_SIZE, TerrainChunk};

/// Terrain already built for the course. No second terrain model or mesh copy is needed.
pub(crate) struct Ground<'a>(pub(crate) &'a [TerrainChunk]);

impl Ground<'_> {
    /// Height of the drawn triangle under a point, including channel beds.
    pub(crate) fn at(&self, east: f64, north: f64) -> Option<f64> {
        self.triangles((east, north), 0.0).find_map(|[a, b, c]| {
            let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if det.abs() < 1e-10 {
                return None;
            }
            let wa = ((b[1] - c[1]) * (east - c[0]) + (c[0] - b[0]) * (north - c[1])) / det;
            let wb = ((c[1] - a[1]) * (east - c[0]) + (a[0] - c[0]) * (north - c[1])) / det;
            let wc = 1.0 - wa - wb;
            (wa.min(wb).min(wc) >= -1e-8).then(|| wa * a[2] + wb * b[2] + wc * c[2])
        })
    }

    /// Lowest drawn ground across a rectangular footing. Clip triangles to the complete
    /// footprint: a channel or a steep face can pass between its corners.
    pub(crate) fn lowest(
        &self,
        centre: (f64, f64),
        forward: (f64, f64),
        (length, width): (f64, f64),
    ) -> Option<f64> {
        lowest(
            self.triangles(centre, length.hypot(width)),
            centre,
            forward,
            (length, width),
        )
    }

    /// Triangles overlapping the footing's bounds, in east/north/height coordinates.
    fn triangles(
        &self,
        (east, north): (f64, f64),
        reach: f64,
    ) -> impl Iterator<Item = [[f64; 3]; 3]> + '_ {
        self.0
            .iter()
            .filter(move |chunk| {
                (f64::from(chunk.center[0]) - east).abs() <= CHUNK_SIZE / 2.0 + reach
                    && (-f64::from(chunk.center[2]) - north).abs() <= CHUNK_SIZE / 2.0 + reach
            })
            .flat_map(move |chunk| {
                chunk
                    .mesh
                    .indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter_map(move |face| {
                        let triangle = [face[0], face[1], face[2]].map(|i| {
                            let v = chunk.mesh.vertices[i as usize];
                            [
                                f64::from(v[0]) + f64::from(chunk.center[0]),
                                -f64::from(v[2]) - f64::from(chunk.center[2]),
                                f64::from(v[1]) + f64::from(chunk.center[1]),
                            ]
                        });
                        let outside = (0..2).any(|axis| {
                            let coordinate = [east, north][axis];
                            triangle.iter().all(|v| v[axis] < coordinate - reach)
                                || triangle.iter().all(|v| v[axis] > coordinate + reach)
                        });
                        (!outside).then_some(triangle)
                    })
            })
    }
}

/// Lowest point where terrain triangles meet a rotated rectangular footing.
pub(crate) fn lowest(
    triangles: impl Iterator<Item = [[f64; 3]; 3]>,
    centre: (f64, f64),
    forward: (f64, f64),
    (length, width): (f64, f64),
) -> Option<f64> {
    let mut lowest = f64::INFINITY;
    for triangle in triangles {
        let mut polygon: Vec<_> = triangle
            .into_iter()
            .map(|[e, n, h]| {
                let (e, n) = (e - centre.0, n - centre.1);
                [
                    e * forward.0 + n * forward.1,
                    e * forward.1 - n * forward.0,
                    h,
                ]
            })
            .collect();
        for (axis, sign, half) in [
            (0, 1.0, length),
            (0, -1.0, length),
            (1, 1.0, width),
            (1, -1.0, width),
        ] {
            let mut clipped = Vec::new();
            for (i, &a) in polygon.iter().enumerate() {
                let b = polygon[(i + 1) % polygon.len()];
                let (fa, fb) = (half - a[axis] * sign, half - b[axis] * sign);
                if fa >= 0.0 {
                    clipped.push(a);
                }
                if (fa >= 0.0) != (fb >= 0.0) {
                    let t = fa / (fa - fb);
                    clipped.push([0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t));
                }
            }
            polygon = clipped;
            if polygon.is_empty() {
                break;
            }
        }
        for point in polygon {
            lowest = lowest.min(point[2]);
        }
    }
    lowest.is_finite().then_some(lowest)
}
