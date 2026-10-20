#!/usr/bin/env sh
# Renders ride screenshots with software Vulkan (lavapipe) on a virtual display, so visuals can
# be checked without a GPU. Runs inside the dev container; images go to screenshots/.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$root/screenshots"
"$root/scripts/build-gdext.sh" debug >/dev/null
# Weeks of made-up rides, so the history and the Analysis tab (R61) have something to show.
if [ "${SCREENSHOT_ERRORS_ONLY:-}" != 1 ]; then
    cargo run -q --manifest-path "$root/core/Cargo.toml" -p orqwa-cli -- sample-rides --weeks 16
fi
godot --headless --path "$root/app" --import >/dev/null 2>&1 || true
# Another window size, e.g. SCREENSHOT_SIZE=1024x640, to check that the layouts reflow (R94).
size="${SCREENSHOT_SIZE:-1600x900}"
# Software rendering is slow; a broken script would otherwise leave Godot running forever.
output="$(SCREENSHOT_DIR="$root/screenshots" timeout 900 xvfb-run -a -s "-screen 0 ${size}x24" \
    godot --path "$root/app" --rendering-driver vulkan --resolution "$size" \
    -s res://tests/screenshots.gd 2>&1)" || { echo "$output"; exit 1; }
echo "$output" | grep -E "saved|world generated|overlay window" || true
if echo "$output" | grep -q "SCRIPT ERROR\|SHADER ERROR\|^ERROR:"; then
    echo "$output" | grep -A2 "SCRIPT ERROR\|SHADER ERROR\|^ERROR:" >&2
    exit 1
fi
