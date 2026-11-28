//! Street imagery for ORQWA (R307, [ADR 0046](../../../docs/adr/0046-street-imagery-mapillary.md)):
//! where photographs of a place were taken, searched along a route and cached on disk, so a
//! course prepared once needs no network to use what it found.
//!
//! The images themselves are not here: a world keeps only the facts they show, and code that
//! names no provider derives them. This crate is the provider side of that seam.

mod corridor;
mod mapillary;

pub use corridor::{CELL_DEGREES, Cell, Located, Side, cells, distance_to_route, locate};
pub use mapillary::{CACHE_VERSION, Mapillary};
