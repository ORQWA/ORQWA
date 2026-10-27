#!/usr/bin/env sh
# Fixed trees, flowers and real materials without course preparation. Run in the dev container.
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${OUT_DIR:-$root/screenshots/season-materials}"
mkdir -p "$out"
"$root/scripts/build-gdext.sh" debug >/dev/null
godot --headless --path "$root/app" --import >/dev/null 2>&1 || true
output="$(SEASON="${SEASON:-Summer}" OUT_DIR="$out" timeout 180 xvfb-run -a \
    godot --path "$root/app" --rendering-driver vulkan --resolution 1280x720 \
    -s res://tools/render_seasons.gd 2>&1)" || { echo "$output"; exit 1; }
echo "$output" | grep '^saved' || true
if echo "$output" | grep -q 'SCRIPT ERROR\|SHADER ERROR\|^ERROR:'; then
    echo "$output" >&2
    exit 1
fi
