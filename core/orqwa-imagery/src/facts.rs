//! What a street photograph says about the buildings beside it (R307, [ADR 0046]).
//!
//! The extractor is deliberately simple and explains itself: colours, darkness and horizontal
//! structure on a coarse grid. It never keeps a pixel; [`StreetFacts`] is all that leaves, and
//! a missing part means the photograph did not show it rather than that the place lacks it.
//!
//! [ADR 0046]: ../../../docs/adr/0046-street-imagery-mapillary.md

use std::collections::BTreeMap;

use orqwa_domain::imagery::{
    FacadeFacts, Frontage, ImageryError, Material, Rgb, RoofFacts, StreetFacts,
};

use crate::select::FrameSide;

/// Width of the grid the picture is reduced to before it is read.
const GRID_WIDTH: usize = 64;

/// Height of the grid the picture is reduced to before it is read.
const GRID_HEIGHT: usize = 48;

/// A decoded picture as 8-bit RGB, three bytes per pixel, row by row.
#[derive(Debug, Clone)]
pub struct Pixels {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl Pixels {
    /// Wraps `rgb` as a `width` by `height` picture.
    ///
    /// # Errors
    /// [`ImageryError::Decode`] if the buffer is empty or not exactly three bytes per pixel.
    pub fn new(width: usize, height: usize, rgb: Vec<u8>) -> Result<Self, ImageryError> {
        if width == 0 || height == 0 || rgb.len() != width.saturating_mul(height).saturating_mul(3)
        {
            return Err(ImageryError::Decode(format!(
                "a {width} by {height} picture needs {} bytes, not {}",
                width.saturating_mul(height).saturating_mul(3),
                rgb.len()
            )));
        }
        Ok(Self { width, height, rgb })
    }

    /// Decodes a JPEG, at most 2048 pixels on either side.
    ///
    /// # Errors
    /// [`ImageryError::Decode`] if the bytes are not a JPEG this build can read.
    pub fn from_jpeg(bytes: &[u8]) -> Result<Self, ImageryError> {
        use zune_jpeg::JpegDecoder;
        use zune_jpeg::zune_core::bytestream::ZCursor;
        use zune_jpeg::zune_core::colorspace::ColorSpace;
        use zune_jpeg::zune_core::options::DecoderOptions;

        let options = DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::RGB)
            .set_max_width(2048)
            .set_max_height(2048);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
        let rgb = decoder
            .decode()
            .map_err(|error| ImageryError::Decode(format!("unreadable JPEG: {error}")))?;
        let info = decoder
            .info()
            .ok_or_else(|| ImageryError::Decode("JPEG has no dimensions".to_owned()))?;
        Self::new(usize::from(info.width), usize::from(info.height), rgb)
    }

    /// Its width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Its height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The picture reduced to the analysis grid.
    fn grid(&self) -> Grid {
        let mut cells = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT);
        for y in 0..GRID_HEIGHT {
            let top = y * self.height / GRID_HEIGHT;
            let bottom = ((y + 1) * self.height / GRID_HEIGHT).max(top + 1);
            for x in 0..GRID_WIDTH {
                let left = x * self.width / GRID_WIDTH;
                let right = ((x + 1) * self.width / GRID_WIDTH).max(left + 1);
                let mut sum = [0_u64; 3];
                let mut count = 0_u64;
                for row in top..bottom.min(self.height) {
                    for column in left..right.min(self.width) {
                        let at = (row * self.width + column) * 3;
                        sum[0] += u64::from(self.rgb[at]);
                        sum[1] += u64::from(self.rgb[at + 1]);
                        sum[2] += u64::from(self.rgb[at + 2]);
                        count += 1;
                    }
                }
                let count = count.max(1);
                #[allow(clippy::cast_precision_loss)] // 8-bit channels
                cells.push(Cell {
                    red: sum[0] as f32 / count as f32 / 255.0,
                    green: sum[1] as f32 / count as f32 / 255.0,
                    blue: sum[2] as f32 / count as f32 / 255.0,
                });
            }
        }
        Grid {
            width: GRID_WIDTH,
            height: GRID_HEIGHT,
            cells,
        }
    }
}

/// Reads the facts a picture shows about the side of the road it was chosen for.
#[must_use]
pub fn derive(pixels: &Pixels, frame: FrameSide) -> StreetFacts {
    let grid = pixels.grid();
    let columns = grid.columns(frame);
    let horizon = grid.horizon();
    let road_top = grid.road_top(horizon, columns);
    let (top, bottom) = grid.facade_rows(horizon, road_top);
    let region = Region {
        top,
        bottom,
        left: columns.0,
        right: columns.1,
    };
    let greenery = grid.greenery(top, columns);
    let kerb = grid.kerb(road_top, columns);

    let Some(wall) = grid.facade_component(region) else {
        return StreetFacts {
            facade: None,
            roof: None,
            frontage: Frontage::Unknown,
            kerb,
            greenery,
        };
    };
    let cells = wall.cells(&grid);
    let bands = wall.window_bands(&grid, wall_luma(&cells));
    let storeys = bands.len().clamp(1, 12);
    let facade = facade(&wall, &cells);
    let roof = grid.roof(&wall, &cells, bands.first().copied());
    let frontage = wall.frontage(&grid);

    StreetFacts {
        facade: facade.map(|(colour, material)| FacadeFacts {
            colour,
            storeys: u8::try_from(storeys).unwrap_or(u8::MAX),
            material,
        }),
        roof,
        frontage,
        kerb,
        greenery,
    }
}

/// A rectangle of the analysis grid.
#[derive(Debug, Clone, Copy)]
struct Region {
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
}

impl Region {
    fn contains(self, x: usize, y: usize) -> bool {
        (self.left..self.right).contains(&x) && (self.top..self.bottom).contains(&y)
    }
}

/// A connected run of wall-coloured cells: a facade, a retaining wall, or a hedge that the
/// colour tests did not catch.
#[derive(Debug, Clone)]
struct Component {
    cells: Vec<(usize, usize)>,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

impl Component {
    fn width(&self) -> usize {
        self.right - self.left
    }

    fn height(&self) -> usize {
        self.bottom - self.top
    }

    fn cells(&self, grid: &Grid) -> Vec<Cell> {
        self.cells.iter().map(|&(x, y)| *grid.at(x, y)).collect()
    }

    /// How much of the component's box it fills. A wall fills most of it; a tree canopy is
    /// ragged and leaves sky showing through.
    #[allow(clippy::cast_precision_loss)] // grid coordinates
    fn solidity(&self) -> f32 {
        let box_cells = self.width() * self.height();
        if box_cells == 0 {
            return 0.0;
        }
        self.cells.len() as f32 / box_cells as f32
    }

    /// How evenly the wall's own cells are lit, as their standard deviation over their mean.
    /// Render and masonry are even; foliage and shadow are not. The darkest cells are left out
    /// so that the openings in a facade do not count against it.
    #[allow(clippy::cast_precision_loss)] // grid coordinates
    fn evenness(&self, grid: &Grid) -> f32 {
        let mut lumas: Vec<f32> = self
            .cells
            .iter()
            .map(|&(x, y)| grid.at(x, y).luma())
            .collect();
        if lumas.is_empty() {
            return f32::MAX;
        }
        lumas.sort_by(f32::total_cmp);
        let kept = &lumas[lumas.len() * 3 / 10..];
        let count = kept.len() as f32;
        let mean = kept.iter().sum::<f32>() / count;
        if mean <= 0.0 {
            return f32::MAX;
        }
        let deviation = (kept.iter().map(|luma| (luma - mean).powi(2)).sum::<f32>() / count).sqrt();
        deviation / mean
    }

    /// Whether the top of the component follows a straight line, as a roofline or the top of
    /// a wall does and a tree canopy does not.
    #[allow(clippy::cast_precision_loss)] // grid coordinates
    fn top_is_straight(&self) -> bool {
        let mut tops: BTreeMap<usize, usize> = BTreeMap::new();
        for &(x, y) in &self.cells {
            tops.entry(x)
                .and_modify(|top| *top = (*top).min(y))
                .or_insert(y);
        }
        if tops.len() < 5 {
            return false;
        }
        let count = tops.len() as f64;
        let (mut sum_x, mut sum_y, mut squares, mut products) = (0.0, 0.0, 0.0, 0.0);
        for (&x, &y) in &tops {
            let (x, y) = (x as f64, y as f64);
            sum_x += x;
            sum_y += y;
            squares += x * x;
            products += x * y;
        }
        let denominator = count * squares - sum_x * sum_x;
        if denominator.abs() < 1e-6 {
            return false;
        }
        let slope = (count * products - sum_x * sum_y) / denominator;
        let intercept = (sum_y - slope * sum_x) / count;
        let residual = tops
            .iter()
            .map(|(&x, &y)| (y as f64 - (intercept + slope * x as f64)).powi(2))
            .sum::<f64>()
            / count;
        residual.sqrt() < 1.5 && slope.abs() < 2.0
    }

    /// Rows of the component's box that look like a row of windows: several dark cells inside
    /// a wall that still fills part of the row.
    fn window_bands(&self, grid: &Grid, wall: f32) -> Vec<usize> {
        let threshold = (wall * 0.62).max(0.10);
        let mut bands = Vec::new();
        let mut in_band = false;
        for y in self.top..self.bottom {
            let mut dark = 0;
            let mut wall_cells = 0;
            let mut all = 0;
            for x in self.left..self.right {
                let cell = grid.at(x, y);
                all += 1;
                if cell.is_sky() || cell.is_green() {
                    continue;
                }
                if cell.luma() < threshold && cell.saturation() < 0.40 {
                    dark += 1;
                } else if cell.luma() >= threshold {
                    wall_cells += 1;
                }
            }
            #[allow(clippy::cast_precision_loss)]
            let dark_share = dark as f32 / all.max(1) as f32;
            #[allow(clippy::cast_precision_loss)]
            let wall_share = wall_cells as f32 / all.max(1) as f32;
            // Windows come in separate openings; a shopfront is one long one.
            let mut runs = 0;
            let mut run = 0;
            let mut longest = 0;
            for x in self.left..self.right {
                let cell = grid.at(x, y);
                let opening = !cell.is_sky()
                    && !cell.is_green()
                    && cell.luma() < threshold
                    && cell.saturation() < 0.40;
                if opening {
                    run += 1;
                    longest = longest.max(run);
                } else {
                    if run >= 2 {
                        runs += 1;
                    }
                    run = 0;
                }
            }
            if run >= 2 {
                runs += 1;
            }
            // A window row is mostly wall with several openings; a shopfront is one large
            // opening in an otherwise plain wall, and still marks the ground floor.
            let is_window = dark >= 3
                && dark_share >= 0.15
                && (wall_share >= 0.25 || dark_share >= 0.50)
                && (runs >= 2 || longest >= 4);
            if is_window && !in_band {
                bands.push(y);
            }
            in_band = is_window;
        }
        bands
    }

    /// What the ground floor shows, from the bottom quarter of the wall.
    fn frontage(&self, grid: &Grid) -> Frontage {
        let height = self.height();
        if height < 4 {
            return Frontage::Unknown;
        }
        let rows = (height / 4).max(2);
        let mut cells = Vec::new();
        for y in self.bottom.saturating_sub(rows)..self.bottom {
            for x in self.left..self.right {
                let cell = grid.at(x, y);
                if !cell.is_sky() && !cell.is_green() {
                    cells.push(*cell);
                }
            }
        }
        if cells.len() < 4 {
            return Frontage::Unknown;
        }
        #[allow(clippy::cast_precision_loss)]
        let count = cells.len() as f32;
        #[allow(clippy::cast_precision_loss)]
        let dark = cells.iter().filter(|cell| cell.luma() < 0.30).count() as f32 / count;
        #[allow(clippy::cast_precision_loss)]
        let bright = cells.iter().filter(|cell| cell.luma() > 0.62).count() as f32 / count;
        #[allow(clippy::cast_precision_loss)]
        let saturated = cells.iter().filter(|cell| cell.saturation() > 0.30).count() as f32 / count;
        let mean = cells.iter().map(|cell| cell.luma()).sum::<f32>() / count;
        let deviation = (cells
            .iter()
            .map(|cell| (cell.luma() - mean).powi(2))
            .sum::<f32>()
            / count)
            .sqrt();

        if saturated > 0.30 && dark > 0.08 {
            Frontage::Awning
        } else if dark > 0.40 && bright > 0.05 {
            Frontage::ShopWindow
        } else if dark > 0.40 && deviation < 0.06 {
            Frontage::Garage
        } else {
            Frontage::Wall
        }
    }
}

/// The wall's base colour and material, if enough of a wall was in view.
fn facade(wall: &Component, cells: &[Cell]) -> Option<(Rgb, Material)> {
    // Openings are darker than the wall; leaving them out keeps the wall's colour and grain
    // from being read as window frames.
    let wall_luma = wall_luma(cells);
    let cells: Vec<Cell> = cells
        .iter()
        .copied()
        .filter(|cell| cell.luma() >= wall_luma * 0.60)
        .collect();
    if cells.len() < 8 {
        return None;
    }
    let mut reds: Vec<u8> = cells.iter().map(|cell| to_byte(cell.red)).collect();
    let mut greens: Vec<u8> = cells.iter().map(|cell| to_byte(cell.green)).collect();
    let mut blues: Vec<u8> = cells.iter().map(|cell| to_byte(cell.blue)).collect();
    let colour = Rgb {
        red: quantise(*median(&mut reds)),
        green: quantise(*median(&mut greens)),
        blue: quantise(*median(&mut blues)),
    };
    Some((colour, material(&cells, wall.width())))
}

/// The picture reduced to a coarse grid, each cell an averaged colour.
#[derive(Debug)]
struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    fn at(&self, x: usize, y: usize) -> &Cell {
        &self.cells[y * self.width + x]
    }

    /// The columns of the frame that show the chosen side.
    fn columns(&self, frame: FrameSide) -> (usize, usize) {
        match frame {
            FrameSide::Left => (0, self.width / 3),
            FrameSide::Right => (self.width * 2 / 3, self.width),
            FrameSide::Both => (0, self.width),
        }
    }

    /// The first row below the sky, or zero when no column has sky above it.
    fn horizon(&self) -> usize {
        let mut depths = Vec::new();
        for x in 0..self.width {
            let mut row = 0;
            while row < self.height && self.at(x, row).is_sky() {
                row += 1;
            }
            if row >= 2 {
                depths.push(row);
            }
        }
        if depths.len() * 3 < self.width {
            return 0;
        }
        *median(&mut depths)
    }

    /// The first row of road from the bottom, walking up until the road ends.
    fn road_top(&self, horizon: usize, columns: (usize, usize)) -> usize {
        let mut row = self.height;
        while row > horizon + 3 {
            let y = row - 1;
            let cells = columns.1 - columns.0;
            let road = (columns.0..columns.1)
                .filter(|&x| self.at(x, y).is_road())
                .count();
            #[allow(clippy::cast_precision_loss)]
            if cells == 0 || (road as f32 / cells as f32) < 0.35 {
                break;
            }
            row -= 1;
        }
        row
    }

    /// The rows between sky and road that hold the buildings, with a fallback for pictures
    /// where neither was found.
    fn facade_rows(&self, horizon: usize, road_top: usize) -> (usize, usize) {
        if road_top > horizon + 6 {
            (horizon, road_top)
        } else {
            (self.height / 4, self.height * 4 / 5)
        }
    }

    /// The largest connected wall with a row of windows in it: a facade rather than a hedge,
    /// a retaining wall or the sky.
    fn facade_component(&self, region: Region) -> Option<Component> {
        let mut seen = vec![false; self.width * self.height];
        let mut best: Option<Component> = None;
        for y in region.top..region.bottom {
            for x in region.left..region.right {
                if seen[y * self.width + x] || !self.at(x, y).is_wall() {
                    continue;
                }
                let component = self.flood(region, x, y, &mut seen);
                if component.cells.len() < 24 || component.width() < 4 || component.height() < 5 {
                    continue;
                }
                // A facade is a solid, evenly lit box with a straight top. Foliage, a hedge and
                // the shadows of a night picture are none of those.
                if component.solidity() < 0.55 || component.evenness(self) > 0.32 {
                    continue;
                }
                if !component.top_is_straight() {
                    continue;
                }
                let wall = wall_luma(&component.cells(self));
                if component.window_bands(self, wall).is_empty() {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|current| component.cells.len() > current.cells.len())
                {
                    best = Some(component);
                }
            }
        }
        best
    }

    /// The wall-coloured cells connected to (x, y), four ways.
    fn flood(&self, region: Region, x: usize, y: usize, seen: &mut [bool]) -> Component {
        let mut component = Component {
            cells: Vec::new(),
            left: x,
            right: x + 1,
            top: y,
            bottom: y + 1,
        };
        let mut stack = vec![(x, y)];
        seen[y * self.width + x] = true;
        while let Some((x, y)) = stack.pop() {
            component.cells.push((x, y));
            component.left = component.left.min(x);
            component.right = component.right.max(x + 1);
            component.top = component.top.min(y);
            component.bottom = component.bottom.max(y + 1);
            for (nx, ny) in [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ] {
                if !region.contains(nx, ny) || seen[ny * self.width + nx] {
                    continue;
                }
                if self.at(nx, ny).is_wall() {
                    seen[ny * self.width + nx] = true;
                    stack.push((nx, ny));
                }
            }
        }
        component
    }

    /// The roof above the highest window band, when it differs from the wall.
    fn roof(
        &self,
        wall: &Component,
        cells: &[Cell],
        first_band: Option<usize>,
    ) -> Option<RoofFacts> {
        let band = first_band?;
        // A roof is a narrow band above the windows, not the whole wall above a shopfront.
        if band <= wall.top + 2 || band - wall.top > wall.height() / 3 {
            return None;
        }
        let mut roof_cells = Vec::new();
        for y in wall.top..band {
            for x in wall.left..wall.right {
                let cell = self.at(x, y);
                if !cell.is_sky() && !cell.is_green() && cell.luma() > 0.10 {
                    roof_cells.push(*cell);
                }
            }
        }
        if roof_cells.len() < 6 {
            return None;
        }
        let colour = median_colour(&roof_cells)?;
        let wall_colour = median_colour(cells)?;
        let difference = [
            i32::from(colour.red) - i32::from(wall_colour.red),
            i32::from(colour.green) - i32::from(wall_colour.green),
            i32::from(colour.blue) - i32::from(wall_colour.blue),
        ]
        .into_iter()
        .map(i32::abs)
        .max()?;
        if difference < 24 {
            return None;
        }
        Some(RoofFacts {
            colour,
            material: roof_material(colour),
        })
    }

    /// Whether a narrow bright band separates the road from the wall.
    fn kerb(&self, road_top: usize, columns: (usize, usize)) -> bool {
        if road_top == 0 || road_top >= self.height {
            return false;
        }
        let row = road_top - 1;
        let cells = columns.1 - columns.0;
        if cells == 0 {
            return false;
        }
        let bright = (columns.0..columns.1)
            .filter(|&x| {
                let cell = self.at(x, row);
                cell.luma() > 0.52 && cell.saturation() < 0.16
            })
            .count();
        #[allow(clippy::cast_precision_loss)]
        let bright = bright as f32 / cells as f32;
        if bright < 0.35 {
            return false;
        }
        // A kerb is one or two rows thick; a light wall continues upwards.
        let above = (columns.0..columns.1)
            .filter(|&x| {
                let cell = self.at(x, row.saturating_sub(1));
                cell.luma() > 0.52 && cell.saturation() < 0.16
            })
            .count();
        #[allow(clippy::cast_precision_loss)]
        let above = above as f32 / cells as f32;
        above < 0.60
    }

    /// Whether greenery stands at street level.
    fn greenery(&self, top: usize, columns: (usize, usize)) -> bool {
        let mut green = 0;
        let mut all = 0;
        for y in top..self.height {
            for x in columns.0..columns.1 {
                all += 1;
                if self.at(x, y).is_green() {
                    green += 1;
                }
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let share = green as f32 / all.max(1) as f32;
        share > 0.06
    }
}

/// One averaged cell of the grid, channels in 0..1.
#[derive(Debug, Clone, Copy)]
struct Cell {
    red: f32,
    green: f32,
    blue: f32,
}

impl Cell {
    fn luma(self) -> f32 {
        self.red
            .mul_add(0.2126, self.green.mul_add(0.7152, self.blue * 0.0722))
    }

    fn saturation(self) -> f32 {
        let max = self.red.max(self.green).max(self.blue);
        let min = self.red.min(self.green).min(self.blue);
        if max <= 0.0 {
            return 0.0;
        }
        (max - min) / max
    }

    fn is_sky(self) -> bool {
        let luma = self.luma();
        // Blue-dominant, or nearly white: a light neutral wall is neither, which keeps a pale
        // plaster facade from being read as sky.
        (self.blue > self.red + 0.01 && self.blue >= self.green && luma > 0.35)
            || (luma > 0.88 && self.saturation() < 0.05)
    }

    fn is_green(self) -> bool {
        // Green, and the olive and yellow-green of autumn foliage and grass, where red and
        // green are close. A warm beige wall keeps a wider gap between green and blue.
        self.green >= self.red * 0.95
            && self.green > self.blue * 1.10
            && self.green >= self.red.max(self.blue)
            && self.saturation() > 0.15
    }

    fn is_wall(self) -> bool {
        let luma = self.luma();
        !self.is_sky() && !self.is_green() && (0.13..=0.92).contains(&luma)
    }

    fn is_road(self) -> bool {
        let luma = self.luma();
        self.saturation() < 0.14 && (0.08..0.62).contains(&luma) && !self.is_green()
    }
}

/// What the wall is made of, from its colour and the direction of its grain.
fn material(cells: &[Cell], width: usize) -> Material {
    let Some(wall) = median_cell(cells) else {
        return Material::Unknown;
    };
    let saturation = wall.saturation();
    let luma = wall.luma();
    let (horizontal, vertical) = grain(cells, width);
    let (red, green, blue) = (wall.red, wall.green, wall.blue);

    if saturation > 0.12
        && red > green
        && green >= blue
        && green > blue * 1.5
        && horizontal > vertical * 1.5
        && horizontal > 0.004
    {
        Material::Wood
    } else if saturation > 0.22 && red > green * 1.12 && red > blue * 1.30 {
        Material::Brick
    } else if saturation < 0.22 && luma > 0.50 && horizontal < 0.09 {
        // Render, painted or not, is the common light wall.
        Material::Plaster
    } else if horizontal > 0.08 {
        Material::Stone
    } else if saturation < 0.20 {
        Material::Concrete
    } else if luma > 0.50 {
        Material::Plaster
    } else {
        Material::Unknown
    }
}

/// The name a material is cached and reported under.
#[must_use]
pub fn material_name(material: Material) -> &'static str {
    match material {
        Material::Brick => "brick",
        Material::Plaster => "plaster",
        Material::Concrete => "concrete",
        Material::Wood => "wood",
        Material::Stone => "stone",
        Material::Metal => "metal",
        Material::Tiles => "tiles",
        Material::Slate => "slate",
        Material::Unknown => "unknown",
    }
}

/// The name a frontage is cached and reported under.
#[must_use]
pub fn frontage_name(frontage: Frontage) -> &'static str {
    match frontage {
        Frontage::ShopWindow => "shop_window",
        Frontage::Awning => "awning",
        Frontage::Garage => "garage",
        Frontage::Wall => "wall",
        Frontage::Unknown => "unknown",
    }
}

/// The material of a roof from its colour.
fn roof_material(colour: Rgb) -> Material {
    let (red, green, blue) = (
        f32::from(colour.red),
        f32::from(colour.green),
        f32::from(colour.blue),
    );
    let luma = red.mul_add(0.2126, green.mul_add(0.7152, blue * 0.0722)) / 255.0;
    if red > green * 1.20 && red > blue * 1.30 {
        Material::Tiles
    } else if luma < 0.30 {
        Material::Slate
    } else if red.max(green).max(blue) - red.min(green).min(blue) < 26.0 && luma > 0.55 {
        Material::Metal
    } else {
        Material::Unknown
    }
}

/// How much the wall's brightness changes from cell to cell, horizontally and vertically.
fn grain(cells: &[Cell], width: usize) -> (f32, f32) {
    let mut horizontal: f32 = 0.0;
    let mut horizontal_pairs: f32 = 0.0;
    for row in cells.chunks(width.max(1)) {
        for pair in row.windows(2) {
            horizontal += (pair[0].luma() - pair[1].luma()).abs();
            horizontal_pairs += 1.0;
        }
    }
    let mut vertical: f32 = 0.0;
    let mut vertical_pairs: f32 = 0.0;
    for pair in cells.windows(width.max(1)) {
        vertical += (pair[0].luma() - pair[1].luma()).abs();
        vertical_pairs += 1.0;
    }
    (
        horizontal / horizontal_pairs.max(1.0),
        vertical / vertical_pairs.max(1.0),
    )
}

/// The brightness of the wall's colour, for telling windows from it.
fn wall_luma(cells: &[Cell]) -> f32 {
    let mut lumas: Vec<f32> = cells.iter().map(|cell| cell.luma()).collect();
    if lumas.is_empty() {
        return 0.5;
    }
    lumas.sort_by(f32::total_cmp);
    lumas[lumas.len() / 2]
}

/// The wall's colour by median channel, which a minority of roof or street does not move.
fn median_cell(cells: &[Cell]) -> Option<Cell> {
    if cells.is_empty() {
        return None;
    }
    let mut reds: Vec<f32> = cells.iter().map(|cell| cell.red).collect();
    let mut greens: Vec<f32> = cells.iter().map(|cell| cell.green).collect();
    let mut blues: Vec<f32> = cells.iter().map(|cell| cell.blue).collect();
    reds.sort_by(f32::total_cmp);
    greens.sort_by(f32::total_cmp);
    blues.sort_by(f32::total_cmp);
    let middle = cells.len() / 2;
    Some(Cell {
        red: reds[middle],
        green: greens[middle],
        blue: blues[middle],
    })
}

fn median_colour(cells: &[Cell]) -> Option<Rgb> {
    if cells.is_empty() {
        return None;
    }
    let mut reds: Vec<u8> = cells.iter().map(|cell| to_byte(cell.red)).collect();
    let mut greens: Vec<u8> = cells.iter().map(|cell| to_byte(cell.green)).collect();
    let mut blues: Vec<u8> = cells.iter().map(|cell| to_byte(cell.blue)).collect();
    Some(Rgb {
        red: quantise(*median(&mut reds)),
        green: quantise(*median(&mut greens)),
        blue: quantise(*median(&mut blues)),
    })
}

/// The middle value, sorting in place.
fn median<T: Ord + Copy>(values: &mut [T]) -> &T {
    values.sort_unstable();
    &values[values.len() / 2]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Rounds a channel to eight steps, so a second reading gives the same value.
fn quantise(channel: u8) -> u8 {
    ((u16::from(channel) / 8) * 8 + 4).min(255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture painted from normalised coordinates.
    #[allow(clippy::cast_precision_loss)] // pixel positions in a test picture
    fn picture(width: usize, height: usize, paint: impl Fn(f64, f64) -> [u8; 3]) -> Pixels {
        let mut rgb = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                #[allow(clippy::cast_precision_loss)]
                let at = (
                    (x as f64 + 0.5) / width as f64,
                    (y as f64 + 0.5) / height as f64,
                );
                rgb.extend_from_slice(&paint(at.0, at.1));
            }
        }
        Pixels::new(width, height, rgb).unwrap()
    }

    const SKY: [u8; 3] = [140, 170, 220];
    const ROAD: [u8; 3] = [90, 90, 92];
    const KERB: [u8; 3] = [205, 205, 202];
    const PLASTER: [u8; 3] = [205, 200, 190];
    const BRICK: [u8; 3] = [165, 85, 60];
    const GLASS: [u8; 3] = [35, 40, 48];

    /// Sky, a wall with `storeys` rows of windows, a kerb and the road.
    #[allow(clippy::cast_precision_loss)] // window positions in a test picture
    fn street(wall: [u8; 3], storeys: usize) -> Pixels {
        picture(640, 480, move |x, y| {
            if y < 0.22 {
                SKY
            } else if y > 0.83 {
                ROAD
            } else if y > 0.80 {
                KERB
            } else {
                let in_window_row = (0..storeys).any(|row| {
                    let top = 0.30 + row as f64 * 0.16;
                    (top..top + 0.08).contains(&y)
                });
                let in_window = x < 0.95 && ((x - 0.05) % 0.16) < 0.09;
                if in_window_row && in_window {
                    GLASS
                } else {
                    wall
                }
            }
        })
    }

    #[test]
    fn a_plaster_wall_with_three_window_rows_has_three_storeys() {
        let facts = derive(&street(PLASTER, 3), FrameSide::Both);
        let facade = facts.facade.expect("a wall was in view");
        assert_eq!(facade.storeys, 3);
        assert_eq!(facade.material, Material::Plaster);
        assert!(
            (i32::from(facade.colour.red) - i32::from(PLASTER[0])).abs() < 12,
            "{:?}",
            facade.colour
        );
        assert_eq!(facts.frontage, Frontage::Wall);
        assert!(!facts.greenery);
    }

    #[test]
    fn a_brick_wall_is_read_as_brick() {
        let facts = derive(&street(BRICK, 1), FrameSide::Both);
        let facade = facts.facade.expect("a wall was in view");
        assert_eq!(facade.material, Material::Brick);
        assert!(facade.colour.red > facade.colour.green);
    }

    #[test]
    fn a_shopfront_at_street_level_is_recognised() {
        let shop = picture(640, 480, |x, y| {
            if y < 0.22 {
                SKY
            } else if y > 0.83 {
                ROAD
            } else if y > 0.80 {
                KERB
            } else if y > 0.62 {
                if (x * 12.0).fract() < 0.12 {
                    [230, 230, 228]
                } else {
                    GLASS
                }
            } else {
                PLASTER
            }
        });
        let facts = derive(&shop, FrameSide::Both);
        assert_eq!(facts.frontage, Frontage::ShopWindow, "{facts:?}");
    }

    #[test]
    fn an_awning_over_a_dark_ground_floor_is_recognised() {
        let awning = picture(640, 480, |_x, y| {
            if y < 0.22 {
                SKY
            } else if y > 0.83 {
                ROAD
            } else if y > 0.80 {
                KERB
            } else if y > 0.72 {
                GLASS
            } else if y > 0.64 {
                [210, 110, 40]
            } else {
                PLASTER
            }
        });
        let facts = derive(&awning, FrameSide::Both);
        assert_eq!(facts.frontage, Frontage::Awning, "{facts:?}");
    }

    #[test]
    fn a_kerb_is_seen_between_the_road_and_a_dark_wall() {
        let facts = derive(&street(BRICK, 2), FrameSide::Both);
        assert!(facts.kerb, "{facts:?}");
    }

    #[test]
    fn each_side_of_the_picture_describes_its_own_wall() {
        let two_walls = picture(640, 480, |x, y| {
            if y < 0.22 {
                SKY
            } else if y > 0.83 {
                ROAD
            } else if y > 0.80 {
                KERB
            } else if (0.34..0.42).contains(&y) && ((x - 0.05) % 0.16) < 0.09 {
                GLASS
            } else if x < 0.5 {
                PLASTER
            } else {
                BRICK
            }
        });
        let left = derive(&two_walls, FrameSide::Left).facade.unwrap();
        let right = derive(&two_walls, FrameSide::Right).facade.unwrap();
        assert_eq!(left.material, Material::Plaster);
        assert_eq!(right.material, Material::Brick);
        assert!(right.colour.green < left.colour.green);
    }

    #[test]
    fn a_roof_above_the_windows_is_read() {
        let roofed = picture(640, 480, |x, y| {
            if y < 0.16 {
                SKY
            } else if y < 0.30 {
                [150, 70, 55]
            } else if y > 0.83 {
                ROAD
            } else if y > 0.80 {
                KERB
            } else if (0.36..0.44).contains(&y) && ((x - 0.05) % 0.16) < 0.09 {
                GLASS
            } else {
                PLASTER
            }
        });
        let facts = derive(&roofed, FrameSide::Both);
        let roof = facts.roof.expect("a roof was in view");
        assert_eq!(roof.material, Material::Tiles, "{facts:?}");
        assert!(roof.colour.red > roof.colour.blue);
    }

    #[test]
    fn a_hedge_is_greenery_and_not_a_wall() {
        let hedge = picture(640, 480, |_x, y| {
            if y < 0.22 {
                SKY
            } else if y > 0.83 {
                ROAD
            } else if y > 0.64 {
                [40, 110, 50]
            } else {
                PLASTER
            }
        });
        let facts = derive(&hedge, FrameSide::Both);
        assert!(facts.greenery, "{facts:?}");
        // A wall with no windows in view is not claimed as a facade.
        assert!(facts.facade.is_none(), "{facts:?}");
    }

    #[test]
    fn a_jpeg_is_decoded_to_its_pixels() {
        let pixels = Pixels::from_jpeg(tests_support::JPEG).unwrap();
        assert_eq!((pixels.width(), pixels.height()), (32, 24));
    }

    #[test]
    fn a_picture_that_is_not_a_jpeg_is_refused() {
        let error = Pixels::from_jpeg(b"not a jpeg").unwrap_err();
        assert!(matches!(error, ImageryError::Decode(_)), "{error}");
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    /// A 32 by 24 solid beige JPEG, made for the decoder tests.
    pub(crate) const JPEG: &[u8] = &[
        255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0, 67,
        0, 3, 2, 2, 3, 2, 2, 3, 3, 3, 3, 4, 3, 3, 4, 5, 8, 5, 5, 4, 4, 5, 10, 7, 7, 6, 8, 12, 10,
        12, 12, 11, 10, 11, 11, 13, 14, 18, 16, 13, 14, 17, 14, 11, 11, 16, 22, 16, 17, 19, 20, 21,
        21, 21, 12, 15, 23, 24, 22, 20, 24, 18, 20, 21, 20, 255, 219, 0, 67, 1, 3, 4, 4, 5, 4, 5,
        9, 5, 5, 9, 20, 13, 11, 13, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20,
        20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20,
        20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 255, 192, 0, 17, 8, 0, 24, 0, 32, 3, 1, 34, 0,
        2, 17, 1, 3, 17, 1, 255, 196, 0, 31, 0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 255, 196, 0, 181, 16, 0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4,
        4, 0, 0, 1, 125, 1, 2, 3, 0, 4, 17, 5, 18, 33, 49, 65, 6, 19, 81, 97, 7, 34, 113, 20, 50,
        129, 145, 161, 8, 35, 66, 177, 193, 21, 82, 209, 240, 36, 51, 98, 114, 130, 9, 10, 22, 23,
        24, 25, 26, 37, 38, 39, 40, 41, 42, 52, 53, 54, 55, 56, 57, 58, 67, 68, 69, 70, 71, 72, 73,
        74, 83, 84, 85, 86, 87, 88, 89, 90, 99, 100, 101, 102, 103, 104, 105, 106, 115, 116, 117,
        118, 119, 120, 121, 122, 131, 132, 133, 134, 135, 136, 137, 138, 146, 147, 148, 149, 150,
        151, 152, 153, 154, 162, 163, 164, 165, 166, 167, 168, 169, 170, 178, 179, 180, 181, 182,
        183, 184, 185, 186, 194, 195, 196, 197, 198, 199, 200, 201, 202, 210, 211, 212, 213, 214,
        215, 216, 217, 218, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 241, 242, 243, 244,
        245, 246, 247, 248, 249, 250, 255, 196, 0, 31, 1, 0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0,
        0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 255, 196, 0, 181, 17, 0, 2, 1, 2, 4, 4, 3, 4,
        7, 5, 4, 4, 0, 1, 2, 119, 0, 1, 2, 3, 17, 4, 5, 33, 49, 6, 18, 65, 81, 7, 97, 113, 19, 34,
        50, 129, 8, 20, 66, 145, 161, 177, 193, 9, 35, 51, 82, 240, 21, 98, 114, 209, 10, 22, 36,
        52, 225, 37, 241, 23, 24, 25, 26, 38, 39, 40, 41, 42, 53, 54, 55, 56, 57, 58, 67, 68, 69,
        70, 71, 72, 73, 74, 83, 84, 85, 86, 87, 88, 89, 90, 99, 100, 101, 102, 103, 104, 105, 106,
        115, 116, 117, 118, 119, 120, 121, 122, 130, 131, 132, 133, 134, 135, 136, 137, 138, 146,
        147, 148, 149, 150, 151, 152, 153, 154, 162, 163, 164, 165, 166, 167, 168, 169, 170, 178,
        179, 180, 181, 182, 183, 184, 185, 186, 194, 195, 196, 197, 198, 199, 200, 201, 202, 210,
        211, 212, 213, 214, 215, 216, 217, 218, 226, 227, 228, 229, 230, 231, 232, 233, 234, 242,
        243, 244, 245, 246, 247, 248, 249, 250, 255, 218, 0, 12, 3, 1, 0, 2, 17, 3, 17, 0, 63, 0,
        250, 86, 138, 40, 175, 156, 62, 128, 40, 162, 138, 0, 40, 162, 138, 0, 40, 162, 138, 0,
        255, 217,
    ];
}
