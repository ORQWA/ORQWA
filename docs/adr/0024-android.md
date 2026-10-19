# ADR 0024 — Torqa on Android tablets

- Status: accepted for a trial (phase A passed; phases B–D to follow, removable as below)
- Date: 2026-10-19

## Context

The riders want Torqa on an Android tablet beside the trainer, not only on a computer. ADR 0019
chose a remote in the phone's browser over a companion app (R86). This is different: the whole app
running on the tablet, with Bluetooth to the trainer and the shifter.

Two things decided whether it is possible at all:

- **Bluetooth.** btleplug, Torqa's Bluetooth crate, supports Android only through Java classes of
  its own (`droidplug`, with the jni-utils classes vendored). They must be in the APK and be found
  through a Java environment that has the app's class loader. Neither btleplug nor Godot
  documents how a Godot extension gets there.
- **The extension.** godot-rust does not maintain Android support. The community tests
  `aarch64-linux-android`, and gdext 0.5.3 fixed classes not registering on Android.

The toolchain (SDK, NDK, Java, Godot's Android templates, FFmpeg for Android) adds about 6 GB to a
build image. That is too much for the dev image, which every check pulls.

## Decision

- **Tablets first, sideloaded from GitHub Releases.** No Play Store or F-Droid for now. Phones
  run it with the low graphics preset.
- **A separate Android image** (`android/Dockerfile`), built on top of the dev image:
  - OpenJDK 21 from Debian. Godot asks for 17 or newer, and Debian 13 has no 17.
  - The SDK's command-line tools 23.0, platform 36 and build tools 36.1.0, as Godot 4.7.2's
    Android build expects.
  - NDK r30, the newest stable one.
  - FFmpeg for Android, through `scripts/build-ffmpeg.sh android` (API 24).
  - Godot's Android templates and a debug keystore.
  - It is built only when an APK is wanted: `scripts/android.sh`, with Gradle's downloads in a
    volume of their own.
- **The project stays as it is.** `scripts/build-android.sh` builds the core for
  `aarch64-linux-android`, then copies `app/` into the container. Only the copy gets:
  - the extension's Android library;
  - Android's texture formats (ETC2/ASTC; in `app/` they would re-import every committed `.import`
    file);
  - the Android export preset;
  - Torqa's Godot Android plugin;
  - Godot's Gradle build, unzipped from the templates, because the editor's
    `--install-android-build-template` stays open.
- **Bluetooth through a small Godot Android plugin** (`android/plugin`, Java):
  - Godot creates its plugins before it opens its extensions, on the app's main thread. The plugin
    loads `libtorqa_gd.so` there, and Torqa's `JNI_OnLoad` hands the Java VM to
    `btleplug::platform::init`.
  - btleplug's Java sources are compiled into the plugin straight from the crate cargo built, so
    they always match. Keep rules stop R8 from stripping them.
  - The plugin asks for `BLUETOOTH_SCAN` (never for location) and `BLUETOOTH_CONNECT`, or for
    location before Android 12.
  - In Rust this takes `torqa-devices::android` and `jni` 0.22 (already btleplug's), both only on
    Android.
- **What Android changes elsewhere:**
  - The core's log goes to Android's log (`adb logcat -s Torqa`), at debug level while support is
    young.
  - A file picked on Android arrives as a `content://` address, which the core cannot open.
    `PickedFile` copies it into Torqa's folder first. Elsewhere it changes nothing.

## Phase A result (2026-10-19)

Tried on a Samsung Galaxy Tab S7 (Snapdragon 865+, Adreno 650) running LineageOS 23.2, which is
Android 16:

- The APK starts in about a second and runs Godot's mobile renderer on Vulkan. Torqa logs
  "Bluetooth ready".
- After the permission prompt, it found and connected the KICKR CORE and the Zwift Click. Power
  came through, and the Click shifted the virtual gears.

Left for the later phases:

- **Smoothness.** It stutters at the Medium preset. SSAO is not available in the mobile renderer.
- **Where data lives.** Profiles land in the cache folder, which Android may clear. They must go to
  the data folder.
- **Saving files.** Saving to a picked place (exporting a ride) does not work yet.
- **HTTPS.** It needs `rustls-platform-verifier`'s Android set-up, which has not been tried yet.
- **CI and releases.** No CI job or release asset yet, and no release signing.

## Phases

- **A** — the trial above.
- **B** — an `android` CI job beside the others, building `Torqa-android-arm64.apk` for releases.
- **C** — fitting Android:
  - storage folders and saving files;
  - a tablet graphics preset;
  - touch;
  - HTTPS;
  - switching off what does not exist there (music control, the overlay window, the MSI updater,
    whose update check can point to the APK instead).
- **D** — a signed release APK, the wiki, and the internet list if anything changes.

## Removing it

Everything Android is in `android/`, `scripts/android.sh`, `scripts/build-android.sh`, the
`android` branch of `scripts/build-ffmpeg.sh` and Rust code marked `cfg(target_os = "android")`.
`android/README.md` lists the steps. `PickedFile` can stay: elsewhere it changes nothing.

## Consequences

- Torqa can run on Android tablets, with a 154 MB APK (debug-signed for now).
- A second image of 10.6 GB, built only for APKs. The dev image and the checks are unchanged.
- Android support rests on btleplug's Java part and on godot-rust's community-tested Android path.
  Either can break with an update; phase B's CI job will tell.
