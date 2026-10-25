#!/usr/bin/env sh
# Counts what a course's world draws, in view and into the shadow maps, under software
# rendering in the dev container (docs/development.md#measuring-frame-rate). Its frame times say
# nothing about a GPU: time frames with scripts/perf/mac.sh.
#   scripts/dev.sh sh -c 'COURSE=<file> PLACES="<m> ..." scripts/perf/software.sh [label]'
# The settings are app/tools/measure_frames.gd's (QUALITY, CAMERA, TAKE, FAR, MINIMAP too), set
# inside `sh -c` so they reach the container. frames.csv, a screenshot of each place and
# <label>.log go to OUT_DIR, screenshots/perf unless set.
set -eu

root="$(cd "$(dirname "$0")/../.." && pwd)"
label="${1:-frames}"
course="$(realpath "${COURSE:?a course file or GPX route}")"
out="$(realpath -m "${OUT_DIR:-$root/screenshots/perf}")"
mkdir -p "$out"
# Release: debug Rust takes many minutes over a city's world.
"$root/scripts/build-gdext.sh" release >/dev/null
godot --headless --path "$root/app" --import >/dev/null 2>&1 || true

status=0
LABEL="$label" OUT_DIR="$out" COURSE="$course" PLACES="${PLACES:?distances along the route}" \
    QUALITY="${QUALITY:-ultra}" CAMERA="${CAMERA:-0}" TAKE="${TAKE:-}" FAR="${FAR:-600}" \
    MINIMAP="${MINIMAP:-close}" timeout 7200 xvfb-run -a -s "-screen 0 1600x900x24" \
    godot --path "$root/app" --rendering-driver vulkan --resolution 1600x900 \
    -s res://tools/measure_frames.gd >"$out/$label.log" 2>&1 || status=$?
grep "^PERF\|SCRIPT ERROR\|SHADER ERROR" "$out/$label.log" || true
exit "$status"
