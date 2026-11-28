//! Street imagery for ORQWA (R307, [ADR 0046](../../../docs/adr/0046-street-imagery-mapillary.md)):
//! where photographs of a place were taken, searched along a route and cached on disk, so a
//! course prepared once needs no network to use what it found.
//!
//! The images themselves are not here: a world keeps only the facts they show, and code that
//! names no provider derives them. This crate is the provider side of that seam.

mod corridor;
mod facts;
mod mapillary;
mod prepare;
mod select;

pub use corridor::{CELL_DEGREES, Cell, Located, Side, cells, distance_to_route, locate};
pub use facts::{Pixels, derive, frontage_name, material_name};
pub use mapillary::{CACHE_VERSION, Mapillary};
pub use prepare::{Derived, FACTS_CACHE, Prepared, THUMBNAIL_CACHE, prepare, thumbnail_path};
pub use select::{Chosen, FrameSide, STRETCH, choose};
