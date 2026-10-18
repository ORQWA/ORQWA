//! Settings of this computer (R78, #257): how detailed the 3D world is drawn (R43), whether
//! the app fills the screen (#218) and the phone remote (#228) depend on the machine in front
//! of the rider, so they live in `computer.toml` next to the cache, never in the data
//! directory, which a rider may share between computers (R30): there the last one to save
//! would win.

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
    /// The phone remote (#228).
    remote: PhoneRemote,
}

/// The phone remote on this computer (#228): a phone pairs with the computer it rides beside.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhoneRemote {
    /// Whether it is on; off until the rider turns it on.
    pub on: bool,
    /// The port it listens on; `None` for the remote's own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// The secret in its address, once it has one; a new one locks out the phones paired
    /// before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
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

/// The phone remote as set on this computer.
#[must_use]
pub fn phone_remote(file: &Path) -> PhoneRemote {
    read(file).remote
}

/// Remembers how the phone remote is set on this computer.
///
/// # Errors
/// On file system errors.
pub fn set_phone_remote(file: &Path, remote: &PhoneRemote) -> Result<(), ProfileError> {
    let mut computer = read(file);
    computer.remote = remote.clone();
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
    fn the_computer_keeps_its_phone_remote_off_until_turned_on() {
        let file = temp_file("remote");
        assert_eq!(
            phone_remote(&file),
            PhoneRemote::default(),
            "off, nothing chosen"
        );
        set_fullscreen(&file, true).unwrap();
        assert!(!phone_remote(&file).on, "other settings leave it off");

        let remote = PhoneRemote {
            on: true,
            port: Some(9000),
            token: Some("0123456789abcdef".to_owned()),
        };
        set_phone_remote(&file, &remote).unwrap();

        assert_eq!(phone_remote(&file), remote);
        assert!(fullscreen(&file), "kept with it");
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("[remote]"), "{text}");
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
