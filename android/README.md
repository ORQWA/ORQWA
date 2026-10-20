# Android (ADR 0024)

Torqa as an APK for Android tablets, built in a container of its own so the dev image and the
checks stay as they are.

```sh
scripts/android.sh scripts/build-android.sh      # → dist/Torqa-android-arm64.apk (debug-signed)
```

| Part | What it is |
|---|---|
| `Dockerfile` | The Android image: the dev image plus OpenJDK 21, the SDK (platform 36, build tools 36.1.0), the NDK, FFmpeg for Android and Godot's Android templates. The debug key is made once and kept in the `orqwa-gradle` volume, so updates install over the APK on a device |
| `plugin/` | Torqa's Godot Android plugin: loads Torqa's library early so btleplug finds its Java classes (compiled in from the btleplug crate), asks for Bluetooth |
| `godot/` | What only the APK build adds to its copy of the project: the export preset and the editor addon that puts the plugin into the export |
| `stage.py` | Adds those, the extension's Android library and Android's texture formats to the copy |

The project in `app/` is never changed: `scripts/build-android.sh` copies it to a build folder in
the container first.

## Installing on a tablet

With wireless debugging paired (`adb pair`, then `adb connect <address>`):

```sh
adb install -r dist/Torqa-android-arm64.apk
adb logcat -s Torqa godot
```

## Release signing

Tagged releases are signed in CI with the release key from the secrets `ANDROID_KEYSTORE` and
`ANDROID_KEYSTORE_PASSWORD` (alias `torqa`). Android installs an update only over an APK with the
same key, so losing it would make every rider uninstall, and lose their rides, to update. Its
owner keeps a copy and the password outside GitHub. Locally, `ORQWA_RELEASE_KEYSTORE` and
`ORQWA_RELEASE_KEYSTORE_PASSWORD` sign the same way.

## Removing Android support

Everything Android lives here and in a few marked places; to take it out again:

1. Delete `android/`, `scripts/android.sh` and `scripts/build-android.sh`.
2. In `scripts/build-ffmpeg.sh`, remove the `android` branch.
3. In `core/`: delete `orqwa-devices/src/android.rs` and its `mod` line, the
   `cfg(target_os = "android")` `jni` dependency in `orqwa-devices/Cargo.toml`, `jni` in the
   workspace `Cargo.toml`, and `JNI_OnLoad` at the end of `orqwa-gd/src/lib.rs`.
4. Remove the Docker volume `orqwa-gradle` and the image `orqwa-android`; in CI, the `android`
   job, `android` in the release's `needs`, the APK in `scripts/release/publish.sh` and its
   test, and the two `ANDROID_KEYSTORE` secrets.
5. Mark ADR 0024 superseded and update `docs/PLAN.md`.
