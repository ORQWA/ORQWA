#!/usr/bin/env sh
# Exports the app's resource pack with app/tools/measure_frames.gd as its main loop, for a copy
# of a native app to measure frame rates with (scripts/perf/mac.sh): release builds take no -s
# or --main-pack. Runs in the dev container, on a staged copy so project.godot stays as it is:
#   scripts/dev.sh scripts/perf/pack.sh [pack] [project]
# `pack` defaults to screenshots/perf/measure.pck. `project` (default app/) is the Godot project
# to pack, e.g. another commit's extracted with `git archive`; it gets today's measuring tool.
set -eu

root="$(cd "$(dirname "$0")/../.." && pwd)"
pack="$(realpath -m "${1:-$root/screenshots/perf/measure.pck}")"
project="$(realpath "${2:-$root/app}")"
stage=/tmp/orqwa-measure-stage

rm -rf "$stage"
mkdir -p "$stage" "$(dirname "$pack")"
cp -a "$project" "$stage/app"
mkdir -p "$stage/app/tools"
cp "$root/app/tools/measure_frames.gd" "$stage/app/tools/measure_frames.gd"
# The import loads the extension; a project extracted from Git has none of its own.
if [ ! -e "$stage/app/bin/liborqwa_gd.so" ]; then
    if [ ! -e "$root/app/bin/liborqwa_gd.so" ]; then
        "$root/scripts/build-gdext.sh" debug >/dev/null
    fi
    mkdir -p "$stage/app/bin"
    cp "$root/app/bin/liborqwa_gd.so" "$stage/app/bin/"
fi
# The engine names its main loop by a global class.
sed -i '1i class_name MeasureFrames' "$stage/app/tools/measure_frames.gd"
sed -i 's|^run/main_scene=|run/main_loop_type="MeasureFrames"\nrun/main_scene=|' \
    "$stage/app/project.godot"

godot --headless --path "$stage/app" --import >/dev/null 2>&1 || true
# A script that does not compile would leave the app waiting for a world forever.
errors="$(godot --headless --path "$stage/app" --check-only -s res://tools/measure_frames.gd 2>&1 \
    | grep -A3 "SCRIPT ERROR\|Parse Error" || true)"
if [ -n "$errors" ]; then
    echo "$errors" >&2
    exit 1
fi
rm -f "$pack"
godot --headless --path "$stage/app" --export-pack "macOS" "$pack" 2>&1 | grep "^ERROR" >&2 || true
test -s "$pack"
rm -rf "$stage"
echo "Exported $pack"
