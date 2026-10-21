//! Controls the music app the rider listens to (R26), with platform helpers:
//! AppleScript on macOS (Spotify or Music), `playerctl` (MPRIS) on Linux and media keys on
//! Windows. Linux needs `playerctl`; macOS asks for permission to control the chosen app.

use crate::failure::{Failure, Kind};
use std::process::Command;

mod commands;

pub use commands::{MediaCommand, command_line};

/// Sends `command`, waiting for the helper program.
///
/// # Errors
/// A readable message if the system has no way to control music, or no player reacted.
pub fn send(command: MediaCommand) -> Result<(), Failure> {
    let (program, args) = command_line(command)
        .ok_or_else(|| Failure::new(Kind::MusicControl, "music control is not supported here"))?;
    let output = Command::new(&program).args(&args).output().map_err(|e| {
        Failure::new(
            Kind::MusicControl,
            format!("cannot control music ({program}: {e})"),
        )
    })?;
    if output.status.success() {
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        Err(Failure::new(
            Kind::MusicResponse,
            format!("no music player reacted: {}", error.trim()),
        ))
    }
}
