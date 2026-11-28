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
}

/// Where imagery is wanted: the route and how far from it to look.
#[derive(Debug, Clone, Copy)]
pub struct Corridor<'a> {
    /// Points along the route, in order.
    pub points: &'a [LatLon],
    /// How far from the route, in metres, an image may lie.
    pub width: f64,
}

/// Why street imagery could not be found.
#[derive(Debug)]
pub enum ImageryError {
    /// The source is unreachable, offline, or was not given a token.
    Unavailable(String),
    /// The source turned the token down.
    Unauthorized(String),
    /// The source refused the request.
    Rejected(String),
    /// The cache could not be read or written.
    Cache(std::io::Error),
}

impl fmt::Display for ImageryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(why) | Self::Unauthorized(why) | Self::Rejected(why) => {
                f.write_str(why)
            }
            Self::Cache(error) => write!(f, "imagery cache: {error}"),
        }
    }
}

impl std::error::Error for ImageryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Unavailable(_) | Self::Unauthorized(_) | Self::Rejected(_) => None,
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
}
