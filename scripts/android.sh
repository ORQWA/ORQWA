#!/usr/bin/env sh
# Runs a command in the Android build container (ADR 0024): the dev image plus the Android SDK,
# NDK and Java, built only when an APK is wanted. Example: scripts/android.sh scripts/build-android.sh
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

docker build -q -t orqwa-dev -f "$root/.devcontainer/Dockerfile" "$root" >/dev/null
docker build -q -t orqwa-android -f "$root/android/Dockerfile" "$root" >/dev/null

commit="$(git -C "$root" rev-parse --short=7 HEAD 2>/dev/null || true)"

. "$root/scripts/give-back-files.sh"

status=0
# The dev container's volumes, plus Gradle's downloads, so builds after the first stay offline.
docker run --rm \
    -e ORQWA_COMMIT="$commit" \
    -v "$root:/workspaces/orqwa" \
    -v orqwa-cargo-registry:/usr/local/cargo/registry \
    -v orqwa-target:/workspaces/orqwa/core/target \
    -v orqwa-cache:/root/.cache \
    -v orqwa-gradle:/root/.gradle \
    -w /workspaces/orqwa \
    orqwa-android "$@" || status=$?
give_back_files orqwa-android
exit "$status"
