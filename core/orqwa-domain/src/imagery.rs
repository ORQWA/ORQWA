//! Street-level imagery as a plugin interface (R307, [ADR 0046]): what photographs of a place
//! can tell a world, without the photographs themselves.
//!
//! A world keeps only facts derived from an image — facade colour, storeys, materials — never
//! the image or anything traced from it. This module is the seam between the world and a
//! source of images, so the source stays swappable and a ride never depends on one.
//!
//! [ADR 0046]: ../../docs/adr/0046-street-imagery-mapillary.md

use std::fmt;
use std::future::Future;
use std::pin::Pin;

/// A position as (latitude, longitude) in degrees.
pub type LatLon = (f64, f64);

/// The future of a source's search, boxed so that the sources of all providers fit one list.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A street-level image as its source describes it: where and when it was taken and which way
/// the camera looked. The image itself is not part of this; a source may fetch it separately.
#[derive(Debug, Clone, PartialEq)]
pub struct StreetImage {
    /// The source's id for the image.
    pub id: String,
    /// Where it was taken.
    pub position: LatLon,
    /// Which way the camera looked, in radians clockwise from north.
    pub heading: f64,
    /// When it was taken, in seconds since the Unix epoch.
    pub captured_at: i64,
    /// Where the source serves a reduced copy of it, if it does. Only the local cache keeps
    /// this; it is never written to a course or world.
    pub thumbnail: Option<String>,
}

/// Where imagery is wanted: the route and how far from it to look.
#[derive(Debug, Clone, Copy)]
pub struct Corridor<'a> {
    /// Points along the route, in order.
    pub points: &'a [LatLon],
    /// How far from the route, in metres, an image may lie.
    pub width: f64,
}

/// A colour as 8-bit red, green and blue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red.
    pub red: u8,
    /// Green.
    pub green: u8,
    /// Blue.
    pub blue: u8,
}

/// What a wall is made of, as far as a photograph can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// Fired clay bricks.
    Brick,
    /// Render or plaster over masonry.
    Plaster,
    /// Fair-faced concrete.
    Concrete,
    /// Timber boards.
    Wood,
    /// Natural stone.
    Stone,
    /// Sheet metal or metal cladding.
    Metal,
    /// Clay roof tiles.
    Tiles,
    /// Slate roofing.
    Slate,
    /// Not enough to tell.
    Unknown,
}

/// What a ground floor shows to the street.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frontage {
    /// A large shop window, usually framed.
    ShopWindow,
    /// An awning or canopy over the ground floor.
    Awning,
    /// A garage or workshop door.
    Garage,
    /// Plain wall or an ordinary entrance.
    Wall,
    /// Not enough to tell.
    Unknown,
}

/// The wall a street shows: its base colour, how many storeys, and what it is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacadeFacts {
    /// The wall's base colour, quantised so that a second preparation gives the same value.
    pub colour: Rgb,
    /// Storeys visible above the ground floor, at least one.
    pub storeys: u8,
    /// What the wall is made of.
    pub material: Material,
}

/// A roof visible above the wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoofFacts {
    /// The roof's colour, quantised like the wall's.
    pub colour: Rgb,
    /// What the roof is made of.
    pub material: Material,
}

/// What one photograph says about the buildings beside the road (R307, [ADR 0046]).
///
/// This is all a world may keep: no image, crop, outline or texture is part of it. Missing
/// parts mean the photograph did not show them, not that the place lacks them.
///
/// [ADR 0046]: ../../docs/adr/0046-street-imagery-mapillary.md
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreetFacts {
    /// The wall facing the road, if one was in view.
    pub facade: Option<FacadeFacts>,
    /// The roof above it, if one was in view.
    pub roof: Option<RoofFacts>,
    /// What the ground floor shows.
    pub frontage: Frontage,
    /// Whether a kerb separates road and wall.
    pub kerb: bool,
    /// Whether greenery (a hedge, a tree, a verge) stands at street level.
    pub greenery: bool,
}

/// Why street imagery could not be found or used.
#[derive(Debug)]
pub enum ImageryError {
    /// The source is unreachable, offline, or was not given a token.
    Unavailable(String),
    /// The source turned the token down.
    Unauthorized(String),
    /// The source asked us to slow down.
    RateLimited(String),
    /// The source refused the request.
    Rejected(String),
    /// An image could not be decoded.
    Decode(String),
    /// The cache could not be read or written.
    Cache(std::io::Error),
}

impl fmt::Display for ImageryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why)
            | Self::Unauthorized(why)
            | Self::RateLimited(why)
            | Self::Rejected(why)
            | Self::Decode(why) => f.write_str(why),
            Self::Cache(error) => write!(f, "imagery cache: {error}"),
        }
    }
}

impl std::error::Error for ImageryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Unavailable(_)
            | Self::Unauthorized(_)
            | Self::RateLimited(_)
            | Self::Rejected(_)
            | Self::Decode(_) => None,
        }
    }
}

/// A source of street-level imagery (R307, [ADR 0046]), such as Mapillary.
///
/// A source finds images, it does not interpret them: what a world keeps from an image is
/// derived later, by code that names no particular provider.
///
/// [ADR 0046]: ../../docs/adr/0046-street-imagery-mapillary.md
pub trait StreetImagery: Send + Sync {
    /// The source's id, e.g. `mapillary`.
    fn id(&self) -> &'static str;

    /// Its name for the rider, e.g. "Mapillary".
    fn name(&self) -> &'static str;

    /// The credit its licence asks for wherever its facts are shown.
    fn attribution(&self) -> &'static str;

    /// Images within the corridor, nearest the route first. `progress` is called with (cells
    /// done, cells total) as the corridor is searched.
    fn along<'a>(
        &'a self,
        corridor: Corridor<'a>,
        progress: &'a mut (dyn FnMut(usize, usize) + Send),
    ) -> BoxFuture<'a, Result<Vec<StreetImage>, ImageryError>>;

    /// A reduced copy of `image`, for deriving facts from it. It is never stored in a world.
    ///
    /// A source that has no reduced copies leaves this unimplemented.
    fn thumbnail<'a>(
        &'a self,
        image: &'a StreetImage,
    ) -> BoxFuture<'a, Result<Vec<u8>, ImageryError>> {
        let _ = image;
        Box::pin(async {
            Err(ImageryError::Unavailable(
                "this source has no reduced copies of its images".to_owned(),
            ))
        })
    }
}
