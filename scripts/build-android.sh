#!/usr/bin/env sh
# Builds the Android APK (ADR 0024) in the Android container: Torqa's core for 64-bit ARM, the
# project copied to a build folder with what only Android needs (its export preset, Torqa's
# Android plugin and Godot's Android build), exported to dist/Torqa-android-arm64.apk.
# The project itself stays as it is: Android's texture formats alone would re-import every
# texture into the committed .import files.
# Usage: scripts/android.sh scripts/build-android.sh [debug|release]
set -eu

profile="${1:-debug}"
root="$(cd "$(dirname "$0")/.." && pwd)"
stage=/tmp/torqa-android
triple=aarch64-linux-android
bin="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin"

case "$profile" in
    debug) cargo_flags="" ;;
    release) cargo_flags="--release" ;;
    *) echo "usage: $0 [debug|release]" >&2; exit 1 ;;
esac

# Torqa's core, with FFmpeg as the image builds it for Android and bindgen reading Android's
# headers. API 24 (Android 7) is the oldest Godot's Android build supports.
# shellcheck disable=SC2086 # cargo_flags is intentionally empty or a single flag
FFMPEG_DIR=/opt/ffmpeg-android \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$bin/aarch64-linux-android24-clang" \
    CC_aarch64_linux_android="$bin/aarch64-linux-android24-clang" \
    AR_aarch64_linux_android="$bin/llvm-ar" \
    BINDGEN_EXTRA_CLANG_ARGS="--target=aarch64-linux-android24 --sysroot=$bin/../sysroot" \
    cargo build --manifest-path "$root/core/Cargo.toml" -p torqa-gd --locked \
    --target "$triple" $cargo_flags

rm -rf "$stage"
mkdir -p "$stage/app"
# The project without its import cache and the desktop libraries; the editor exporting it needs
# this system's extension to load Torqa's classes.
tar -C "$root/app" --exclude=./.godot --exclude=./bin --exclude=./android -cf - . |
    tar -C "$stage/app" -xf -
"$root/scripts/build-gdext.sh" release >/dev/null
mkdir -p "$stage/app/bin/android"
cp "$root/app/bin/libtorqa_gd.so" "$stage/app/bin/"
"$bin/llvm-strip" -o "$stage/app/bin/android/libtorqa_gd.so" \
    "$root/core/target/$triple/$profile/libtorqa_gd.so"
cp -r "$root/android/godot/addons" "$stage/app/"
python3 "$root/android/stage.py" "$stage/app" "$root/android/godot/export_preset.cfg" \
    "$root/core/Cargo.lock"

# Godot's Android build, then Torqa's plugin built with its Gradle, btleplug's Java sources
# taken from the crate cargo built.
# What the editor's "Install Android Build Template" does, without the editor that stays open.
mkdir -p "$stage/app/android/build"
templates="/root/.local/share/godot/export_templates/$(godot --version | cut -d. -f1-3).stable"
unzip -q "$templates/android_source.zip" -d "$stage/app/android/build"
cp "$templates/version.txt" "$stage/app/android/.build_version"
touch "$stage/app/android/build/.gdignore"
btleplug="$(cargo metadata --format-version 1 --manifest-path "$root/core/Cargo.toml" |
    python3 -c 'import json, os, sys
for p in json.load(sys.stdin)["packages"]:
    if p["name"] == "btleplug":
        print(os.path.join(os.path.dirname(p["manifest_path"]), "src/droidplug/java/src/main/java"))')"
cp -r "$root/android/plugin" "$stage/plugin"
(cd "$stage/app/android/build" && sh ./gradlew --quiet -p "$stage/plugin" assembleRelease \
    -Pbtleplug="$btleplug")
mkdir -p "$stage/app/addons/torqa_android/bin"
cp "$stage/plugin/build/outputs/aar/torqa-android-release.aar" \
    "$stage/app/addons/torqa_android/bin/"

godot --headless --path "$stage/app" --import >/dev/null 2>&1 || true
mkdir -p "$root/dist"
apk="$root/dist/Torqa-android-arm64.apk"
rm -f "$apk"
GODOT_ANDROID_KEYSTORE_DEBUG_PATH=/root/debug.keystore \
    GODOT_ANDROID_KEYSTORE_DEBUG_USER=androiddebugkey \
    GODOT_ANDROID_KEYSTORE_DEBUG_PASSWORD=android \
    godot --headless --path "$stage/app" --export-debug "Android" "$apk"
test -s "$apk" || { echo "no APK exported" >&2; exit 1; }
echo "Built $apk"
