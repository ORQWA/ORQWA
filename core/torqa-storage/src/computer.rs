//! Settings of this computer (R78, #257): how detailed the 3D world is drawn (R43) and whether
//! the app fills the screen (#218) depend on the machine in front of the rider, so they live
//! in `computer.toml` next to the cache, never in the data directory, which a rider may share
//! between computers (R30): there the last one to save would win.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::profiles::{GraphicsQuality, ProfileError, write_atomically};

/// The file's name, in the cache directory.
pub const FILE: &str = "computer.toml";

/// The file format. Missing fields take defaults.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ComputerFile {
    /// How detailed the 3D world is drawn on this computer (R43).
    graphics_quality: Option<GraphicsQuality>,
    /// Whether the app fills the screen (#218).
    fullscreen: Option<bool>,
}

/// The graphics quality chosen on this computer; Medium until one is chosen.
#[must_use]
pub fn graphics_quality(file: &Path) -> GraphicsQuality {
    read(file).graphics_quality.unwrap_or_default()
}

/// Remembers the graphics quality for this computer.
///
/// # Errors
/// On file system errors.
pub fn set_graphics_quality(file: &Path, quality: GraphicsQuality) -> Result<(), ProfileError> {
    let mut computer = read(file);
    computer.graphics_quality = Some(quality);
    save(file, &computer)
}

/// Whether the app fills the screen on this computer (#218).
#[must_use]
pub fn fullscreen(file: &Path) -> bool {
    read(file).fullscreen.unwrap_or(false)
}

/// Remembers whether the app fills the screen, to start that way next time.
///
/// # Errors
/// On file system errors.
pub fn set_fullscreen(file: &Path, fullscreen: bool) -> Result<(), ProfileError> {
    let mut computer = read(file);
    computer.fullscreen = Some(fullscreen);
    save(file, &computer)
}

/// Takes over the settings a data directory from before R78 kept for every computer, where
/// this computer has not chosen its own yet.
pub(crate) fn take_over(
    file: &Path,
    quality: Option<GraphicsQuality>,
    fullscreen: Option<bool>,
) -> Result<(), ProfileError> {
    let mut computer = read(file);
    computer.graphics_quality = computer.graphics_quality.or(quality);
    computer.fullscreen = computer.fullscreen.or(fullscreen);
    save(file, &computer)
}

fn read(file: &Path) -> ComputerFile {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(file: &Path, computer: &ComputerFile) -> Result<(), ProfileError> {
    write_atomically(file, &toml::to_string_pretty(computer)?)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("torqa-computer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(FILE)
    }

    #[test]
    fn the_computer_keeps_its_graphics_quality_and_full_screen() {
        let file = temp_file("settings");
        assert_eq!(graphics_quality(&file), GraphicsQuality::Medium);
        assert!(!fullscreen(&file), "a window until the rider chooses");

        set_graphics_quality(&file, GraphicsQuality::Ultra).unwrap();
        set_fullscreen(&file, true).unwrap();

        assert_eq!(graphics_quality(&file), GraphicsQuality::Ultra);
        assert!(fullscreen(&file));
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains(r#"graphics_quality = "ultra""#), "{text}");
        set_fullscreen(&file, false).unwrap();
        assert!(!fullscreen(&file));
        assert_eq!(
            graphics_quality(&file),
            GraphicsQuality::Ultra,
            "kept with it"
        );
    }

    #[test]
    fn settings_taken_over_never_replace_the_computers_own() {
        let file = temp_file("take-over");
        set_fullscreen(&file, false).unwrap();

        take_over(&file, Some(GraphicsQuality::Low), Some(true)).unwrap();

        assert_eq!(
            graphics_quality(&file),
            GraphicsQuality::Low,
            "none chosen yet"
        );
        assert!(!fullscreen(&file), "chosen before");
        take_over(&file, Some(GraphicsQuality::Ultra), None).unwrap();
        assert_eq!(graphics_quality(&file), GraphicsQuality::Low);
    }
}
