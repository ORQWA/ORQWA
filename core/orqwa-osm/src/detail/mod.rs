//! Map detail (ADR 0033): what the vector tiles leave out of OpenStreetMap — road surfaces such
//! as cobbles, and the way traffic goes round a roundabout — made from the download of a
//! region (`.osm.pbf`, from Geofabrik) a rider asked for, and kept in the map cache.

mod extract;
mod pbf;

pub use extract::{Extract, Stage, VERSION, extract};
