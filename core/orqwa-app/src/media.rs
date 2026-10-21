//! Controls the music app the rider listens to (R26), with what each system offers out of the
//! box: AppleScript on macOS (Spotify or Music), `playerctl` (MPRIS) on Linux and media keys on
//! Windows. Linux needs `playerctl`; macOS asks for permission to control the chosen app.

use crate::failure::{Failure, Kind};
use std::process::Command;

/// What to tell the music app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    /// Play or pause.
    PlayPause,
    /// Next track.
    Next,
    /// Previous track.
    Previous,
}

/// The program and arguments that send `command` on this system, if supported.
#[must_use]
pub fn command_line(command: MediaCommand) -> Option<(String, Vec<String>)> {
    if cfg!(target_os = "macos") {
        Some((
            "osascript".to_owned(),
            vec!["-e".to_owned(), apple_script(command)],
        ))
    } else if cfg!(windows) {
        Some((
            "powershell".to_owned(),
            vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                windows_media_key(command),
            ],
        ))
    } else if cfg!(target_os = "linux") {
        let verb = match command {
            MediaCommand::PlayPause => "play-pause",
            MediaCommand::Next => "next",
            MediaCommand::Previous => "previous",
        };
        Some(("playerctl".to_owned(), vec![verb.to_owned()]))
    } else {
        None
    }
}

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

/// Talks to Spotify if it runs, else to Apple Music; `is running` never launches an app.
fn apple_script(command: MediaCommand) -> String {
    let verb = match command {
        MediaCommand::PlayPause => "playpause",
        MediaCommand::Next => "next track",
        MediaCommand::Previous => "previous track",
    };
    // A tell statement loads the player's dictionary at compile time, even in a branch
    // that won't run. Compile only the selected player's command so Music works without
    // Spotify installed (#260). Bundle IDs also avoid localized application names.
    format!(
        "if application id \"com.spotify.client\" is running then\n\
         run script \"tell application id \\\"com.spotify.client\\\" to {verb}\"\n\
         else if application id \"com.apple.Music\" is running then\n\
         run script \"tell application id \\\"com.apple.Music\\\" to {verb}\"\n\
         else\n\
         error \"neither Spotify nor Music is running\"\n\
         end if"
    )
}

const WINDOWS_MEDIA_KEYS: &str = include_str!("media_keys.ps1");

/// Presses a media key, which the active player handles.
fn windows_media_key(command: MediaCommand) -> String {
    // Virtual-key codes of the media keys.
    let key = match command {
        MediaCommand::PlayPause => 179,
        MediaCommand::Next => 176,
        MediaCommand::Previous => 177,
    };
    format!("{WINDOWS_MEDIA_KEYS}\n[OrqwaMediaKeys]::Press({key})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_script_defers_player_commands_until_after_running_checks() {
        for (command, verb) in [
            (MediaCommand::PlayPause, "playpause"),
            (MediaCommand::Next, "next track"),
            (MediaCommand::Previous, "previous track"),
        ] {
            let script = apple_script(command);
            assert!(
                script.starts_with(r#"if application id "com.spotify.client" is running then"#)
            );
            assert!(script.contains(r#"else if application id "com.apple.Music" is running then"#));
            for id in ["com.spotify.client", "com.apple.Music"] {
                assert!(script.contains(&format!(
                    r#"run script "tell application id \"{id}\" to {verb}""#
                )));
            }
        }
    }

    #[test]
    fn windows_presses_the_matching_media_key() {
        for (command, key) in [
            (MediaCommand::PlayPause, 179),
            (MediaCommand::Next, 176),
            (MediaCommand::Previous, 177),
        ] {
            assert!(
                windows_media_key(command).ends_with(&format!("[OrqwaMediaKeys]::Press({key})"))
            );
        }
    }

    // Native CI runs this: it must reach our no-player error even when neither player's
    // dictionary exists, without launching or sending events to the runner's apps.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_missing_players_report_a_runtime_error_not_a_syntax_error() {
        for command in [
            MediaCommand::PlayPause,
            MediaCommand::Next,
            MediaCommand::Previous,
        ] {
            let script = apple_script(command)
                .replace("com.spotify.client", "org.orqwa.test.missing-spotify")
                .replace("com.apple.Music", "org.orqwa.test.missing-music");
            let output = Command::new("osascript")
                .args(["-e", &script])
                .output()
                .unwrap();
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success());
            assert!(
                error.contains("neither Spotify nor Music is running"),
                "{error}"
            );
        }
    }

    // Compile the real P/Invoke helper and check its ABI and events without injecting keys
    // into the runner's desktop or needing a registered media session.
    #[cfg(windows)]
    #[test]
    fn windows_input_matches_the_native_abi_and_releases_each_key() {
        let script = format!(
            r#"{WINDOWS_MEDIA_KEYS}
$expectedSize = if ([IntPtr]::Size -eq 8) {{ 40 }} else {{ 28 }}
if ([Runtime.InteropServices.Marshal]::SizeOf([type][OrqwaMediaKeys+Input]) -ne $expectedSize) {{
    throw 'Wrong INPUT size'
}}
foreach ($key in @(179, 176, 177)) {{
    $events = [OrqwaMediaKeys]::Events($key)
    if ($events.Length -ne 2) {{ throw 'Expected key down and key up' }}
    foreach ($event in $events) {{
        if ($event.Type -ne 1 -or $event.Data.Keyboard.VirtualKey -ne $key) {{
            throw 'Wrong input type or virtual key'
        }}
        if ($event.Data.Keyboard.ScanCode -ne 0) {{ throw 'Unexpected scan code' }}
    }}
    if ($events[0].Data.Keyboard.Flags -ne 1 -or $events[1].Data.Keyboard.Flags -ne 3) {{
        throw 'Expected extended key down followed by extended key up'
    }}
}}
"#
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn this_system_has_a_way_to_control_music() {
        // The container is Linux: MPRIS via playerctl.
        let (program, args) = command_line(MediaCommand::PlayPause).unwrap();
        if cfg!(target_os = "linux") {
            assert_eq!(program, "playerctl");
            assert_eq!(args, ["play-pause"]);
        }
    }
}
