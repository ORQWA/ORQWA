//! The regions a rider downloaded: each region's download (`<id>.osm.pbf`) and its record
//! (`<id>.json`) in the map cache's `geofabrik` directory, its extract under [`VERSION`].

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::extract::{Stage, VERSION, extract, file_name};
use crate::OsmError;

/// The directory of downloads and their records in the map cache.
pub const DOWNLOADS: &str = "geofabrik";

/// A region whose map detail is ready, as its record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Geofabrik's name for it, e.g. `switzerland` or `bayern`.
    pub id: String,
    /// Its name to show, e.g. `Switzerland`.
    pub name: String,
    /// Its extract's directory under [`VERSION`], e.g. `switzerland-2026-10-24`.
    pub extract: String,
    /// When its data was current (Unix seconds), if the download says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_time: Option<i64>,
}

/// Where region `id`'s download goes in the map cache `osm_dir`.
#[must_use]
pub fn download_path(osm_dir: &Path, id: &str) -> PathBuf {
    osm_dir
        .join(DOWNLOADS)
        .join(format!("{}.osm.pbf", file_name(id)))
}

fn record_path(osm_dir: &Path, id: &str) -> PathBuf {
    osm_dir
        .join(DOWNLOADS)
        .join(format!("{}.json", file_name(id)))
}

/// The regions whose map detail is ready in the map cache `osm_dir`, by id; a record whose
/// extract is gone counts for none.
#[must_use]
pub fn installed(osm_dir: &Path) -> Vec<Region> {
    let Ok(entries) = fs::read_dir(osm_dir.join(DOWNLOADS)) else {
        return Vec::new();
    };
    let mut regions: Vec<Region> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| serde_json::from_slice(&fs::read(path).ok()?).ok())
        .filter(|region: &Region| osm_dir.join(VERSION).join(&region.extract).is_dir())
        .collect();
    regions.sort_by(|a, b| a.id.cmp(&b.id));
    regions
}

/// Makes the map detail of region `id` from its download (see [`download_path`]) and records
/// it as ready under `name`. The extract of an earlier download of the region goes: courses
/// prepared with it carry what they used.
///
/// # Errors
/// [`OsmError::Cache`] if the download cannot be read or the detail written,
/// [`OsmError::Invalid`] if it is no OpenStreetMap download this can read.
pub fn install(
    osm_dir: &Path,
    id: &str,
    name: &str,
    progress: &mut dyn FnMut(Stage, u64, u64),
) -> Result<Region, OsmError> {
    let made = extract(
        &download_path(osm_dir, id),
        &osm_dir.join(VERSION),
        id,
        progress,
    )?;
    let record = record_path(osm_dir, id);
    let earlier: Option<Region> = fs::read(&record)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let region = Region {
        id: id.to_owned(),
        name: name.to_owned(),
        extract: made.name,
        data_time: made.data_time,
    };
    let json = serde_json::to_vec_pretty(&region).map_err(|e| OsmError::Invalid(e.to_string()))?;
    let aside = record.with_extension("json.part");
    fs::write(&aside, json)?;
    fs::rename(&aside, &record)?;
    if let Some(earlier) = earlier
        && earlier.extract != region.extract
    {
        let old = osm_dir.join(VERSION).join(file_name(&earlier.extract));
        if old.is_dir() {
            fs::remove_dir_all(old)?;
        }
    }
    Ok(region)
}

#[cfg(test)]
mod tests {
    use super::super::extract::tests::{TestWay, test_file};
    use super::*;

    fn cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orqwa-regions-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(DOWNLOADS)).unwrap();
        dir
    }

    const SETT: [TestWay; 1] = [TestWay {
        id: 10,
        tags: &[("highway", "residential"), ("surface", "sett")],
        nodes: &[1, 2],
    }];

    #[test]
    fn an_installed_region_is_listed_with_its_extract() {
        let osm = cache("install");
        assert_eq!(installed(&osm), []);
        fs::write(download_path(&osm, "bern"), test_file(&SETT, 1_792_875_600)).unwrap();

        let region = install(&osm, "bern", "Bern", &mut |_, _, _| {}).unwrap();

        assert_eq!(region.extract, "bern-2026-10-24");
        assert_eq!(installed(&osm), vec![region]);
        fs::remove_dir_all(osm).unwrap();
    }

    #[test]
    fn a_newer_download_replaces_the_extract_of_the_earlier() {
        let osm = cache("update");
        fs::write(download_path(&osm, "bern"), test_file(&SETT, 1_792_875_600)).unwrap();
        install(&osm, "bern", "Bern", &mut |_, _, _| {}).unwrap();
        // A day later.
        fs::write(download_path(&osm, "bern"), test_file(&SETT, 1_792_962_000)).unwrap();

        let region = install(&osm, "bern", "Bern", &mut |_, _, _| {}).unwrap();

        assert_eq!(region.extract, "bern-2026-10-25");
        assert_eq!(installed(&osm), vec![region]);
        let extracts: Vec<_> = fs::read_dir(osm.join(VERSION)).unwrap().collect();
        assert_eq!(extracts.len(), 1);
        fs::remove_dir_all(osm).unwrap();
    }

    #[test]
    fn a_region_without_its_extract_is_not_ready() {
        let osm = cache("gone");
        fs::write(download_path(&osm, "bern"), test_file(&SETT, 1_792_875_600)).unwrap();
        let region = install(&osm, "bern", "Bern", &mut |_, _, _| {}).unwrap();
        fs::remove_dir_all(osm.join(VERSION).join(&region.extract)).unwrap();

        assert_eq!(installed(&osm), []);
        fs::remove_dir_all(osm).unwrap();
    }
}
