# ADR 0024 — ORQWA on Android tablets

- Status: accepted (phases A–E done; removable as below)
- Date: 2026-10-19

## Context

The riders want ORQWA on an Android tablet beside the trainer, not only on a computer. ADR 0019
chose a remote in the phone's browser over a companion app (R86). This is different: the whole app
running on the tablet, with Bluetooth to the trainer and the shifter.

Two things decided whether it is possible at all:

- **Bluetooth.** btleplug, ORQWA's Bluetooth crate, supports Android only through Java classes of
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
  - Godot's Android templates. The debug key is not in the image: it is made once in the Gradle
    volume, so a rebuilt image still signs alike and an update installs over the last APK.
  - It is built only when an APK is wanted: `scripts/android.sh`, with Gradle's downloads in a
    volume of their own.
- **The project stays as it is.** `scripts/build-android.sh` builds the core for
  `aarch64-linux-android`, then copies `app/` into the container. Only the copy gets:
  - the extension's Android library;
  - Android's texture formats (ETC2/ASTC; in `app/` they would re-import every committed `.import`
    file);
  - the Android export preset;
  - ORQWA's Godot Android plugin;
  - Godot's Gradle build, unzipped from the templates, because the editor's
    `--install-android-build-template` stays open.
- **Bluetooth through a small Godot Android plugin** (`android/plugin`, Java):
  - Godot creates its plugins before it opens its extensions, on the app's main thread. The plugin
    loads `liborqwa_gd.so` there, and ORQWA's `JNI_OnLoad` hands the Java VM to
    `btleplug::platform::init`.
  - btleplug's Java sources are compiled into the plugin straight from the crate cargo built, so
    they always match. Keep rules stop R8 from stripping them.
  - The plugin asks for `BLUETOOTH_SCAN` (never for location) and `BLUETOOTH_CONNECT`, or for
    location before Android 12.
  - In Rust this takes `orqwa-devices::android` and `jni` 0.22 (already btleplug's), both only on
    Android.
- **What Android changes elsewhere:**
  - The core's log goes to Android's log (`adb logcat -s ORQWA`), at debug level while support is
    young.
  - A file picked on Android arrives as a `content://` address, which the core cannot open.
    `PickedFile` copies it into ORQWA's folder first. Elsewhere it changes nothing.

## Phase A result (2026-10-19)

Tried on a Samsung Galaxy Tab S7 (Snapdragon 865+, Adreno 650) running LineageOS 23.2, which is
Android 16:

- The APK starts in about a second and runs Godot's mobile renderer on Vulkan. ORQWA logs
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
- **B** — an `android` CI job beside the others, building the APK on every pull request in its
  own image (cached in GHCR like the dev image); it joins the releases only with D.
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

- ORQWA can run on Android tablets, with a 154 MB APK (debug-signed for now).
- A second image of 10.6 GB, built only for APKs. The dev image and the checks are unchanged.
- Android support rests on btleplug's Java part and on godot-rust's community-tested Android path.
  Either can break with an update; phase B's CI job will tell.

## Amendment 2026-10-20: phases B and C

- **B.** The `android` CI job builds the APK on every pull request in its own image. That image is
  tagged by the dev image it starts from and the Android Dockerfile, cached in GHCR. It first
  makes room on the runner by removing its own Android SDK, .NET and Haskell.
- **C:**
  - **Data.** Android gives no `HOME` or `XDG_*` directories, so data and cache both fell back to
    the temporary directory, which is the app's cache. Data now goes to the app's `files` folder
    beside it.
  - **Graphics.** Low is the default preset on Android.
  - **Saving.** Saving to a picked `content://` place goes through a file in ORQWA's folder, which
    Godot then copies there.
  - **HTTPS.** reqwest checks certificates through rustls-platform-verifier, which on Android
    needs the app's context. The plugin hands it over once (`initTls`). Its Java part
    (`org.rustls:rustls-platform-verifier`, in the version Cargo.lock names) comes from the
    project's own Maven archive on GitHub. That is a build-time download only; the app contacts
    nothing new.
  - **The overlay (R55) is picture-in-picture.** Android has one window per app, and Godot's
    activity already allows picture-in-picture. The plugin's `enterPictureInPicture` puts it
    there, shaped like the ride's HUD-only layout (within Android's 1:2.39 to 2.39:1), and the
    layout is scaled to the small window. Once the rider opens ORQWA again, the ride screen
    comes back.

## Amendment 2026-10-20: phase C tried

On the Tab S7, with the KICKR CORE and the Zwift Click: a GPX imported on the tablet fetched its
map and terrain tiles over HTTPS, a ride at Low ran smoothly, and picture-in-picture showed the
HUD over other apps. Behind the HUD the window would still draw the world, so picture-in-picture
switches 3D off as the overlay window does, and fills the window in the panels' colour. Android
sizes the window and takes taps on it for its own buttons, so the overlay's bar (sizes, pause,
full view) and resize grip are left out: only the figures show. The debug key moved out of the image into the Gradle
volume: a key made with each image would refuse every update after a rebuild.

## Amendment 2026-10-20: phase D

- **Release signing.** A tagged release's APK is signed with ORQWA's release key (alias `torqa`),
  kept in the repository secrets `ANDROID_KEYSTORE` and `ANDROID_KEYSTORE_PASSWORD` and written to
  the runner only for that step. Every other build, in CI or locally, is signed with a debug key:
  switching between the two needs an uninstall, and so would a lost release key for every rider.
  The key's owner keeps a copy outside GitHub.
- **Version.** The APK carries ORQWA's version: name `0.5.0`, code `500` (major × 10000 +
  minor × 100 + patch), so each release installs over the last.
- **Release asset.** `ORQWA-android-arm64.apk` is published with the others; the release waits
  for the Android build.
- **Updates.** The update check offers the release page, as on macOS and Linux; the rider
  installs the newer APK over the old one. (Replaced by phase E below.)

## Amendment 2026-10-21: phase E, updates from ORQWA (R103)

Riders found installing each APK by hand a chore, and an app store such as Obtainium one app too
many. So ORQWA installs its updates on Android as on Windows (ADR 0023):

- **Download.** The core picks the release's `ORQWA-<version>-android-arm64.apk` on Android
  (ADR 0023, #322), and downloads it into the cache (`updates/ORQWA-<version>.apk`), as it does the MSI on Windows.
- **Install.** The core cannot start Android's installer, so the plugin does: it writes the APK
  into a `PackageInstaller` session for ORQWA's own package and commits it. From Android 12 the
  session asks for no prompt (`UPDATE_PACKAGES_WITHOUT_USER_ACTION`), which Android grants an app
  updating itself; where it asks anyway, its prompt is shown. Android closes ORQWA to update it
  and does not start it again, so the dialog tells the rider to open ORQWA again.
- **Permission.** Installing needs `REQUEST_INSTALL_PACKAGES`, which the rider allows once under
  **Install unknown apps**; the plugin opens that setting and goes on when the rider comes back.
  A failure, from the session or from Android's answer, shows in the update dialog.
- **Advanced Protection.** Android 16's Advanced Protection, like a device administrator's
  policy, forbids every app to install apps (`DISALLOW_INSTALL_UNKNOWN_SOURCES_GLOBALLY`) and
  allows no exception for one app. The plugin asks for that restriction before the download, and
  the dialog then says to turn Advanced Protection off instead of offering the update.
- **Tried** on the Tab S7 (Android 16) on 2026-10-22, with a stand-in for GitHub's latest release
  serving a build signed alike: the update downloaded, the rider allowed installing apps once, and
  Android installed it without a further prompt and closed ORQWA. With Advanced Protection on,
  Android refused the setting, which the restriction check now heads off.
- **Signing.** Android installs an update only when it is signed like the installed app, so only
  release builds update from releases; a CI or debug build is refused and says so.
- **Removal.** The two permissions and the plugin's installer go with the plugin; the core's
  Android installer name and `can_install` go with the other `target_os = "android"` code.
