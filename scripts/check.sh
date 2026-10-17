#!/usr/bin/env sh
# All checks that must pass before a change is done. Runs inside the dev container.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

cd "$root/core"
python3 "$root/scripts/wiki/check.py"
python3 -m unittest discover -s "$root/scripts/wiki" -p 'test_*.py'
python3 -m unittest discover -s "$root/scripts/release" -p 'test_*.py'
# The Cargo workspace and the macOS bundle name the same version; release tags check it too.
python3 "$root/scripts/release/version.py"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny --all-features check
# The crates' licence notices the About tab shows are those of today's dependencies (R83).
"$root/scripts/notices.sh" --check
# Translatable texts: template up to date, every translation complete (R24).
python3 "$root/scripts/i18n/extract.py" --check
python3 -m unittest discover -s "$root/scripts/i18n" -p 'test_*.py'

cd "$root/app"
gdlint .
gdformat --check .

# Godot checks: the extension loads, all scripts compile (warnings are errors) and the
# Torqa node API works end to end.
"$root/scripts/build-gdext.sh" debug
godot --headless --path "$root/app" --import >/dev/null 2>&1 || true

run_godot() {
    output="$(godot --headless --path "$root/app" "$@" 2>&1)" || { echo "$output"; return 1; }
    echo "$output"
    if echo "$output" | grep -q "SCRIPT ERROR\|SHADER ERROR\|^ERROR:"; then
        # The callers capture standard output, so the errors go to standard error as well:
        # otherwise a failing check only says that Godot reported errors, not which.
        echo "$output" | grep -A3 "SCRIPT ERROR\|SHADER ERROR\|^ERROR:" >&2
        echo "Godot reported errors" >&2
        return 1
    fi
}
# Capture first: in a pipeline, sh would only see grep's exit status.
main_output="$(run_godot --quit-after 10)"
echo "$main_output" | grep -q "^Torqa "
echo "Main scene smoke test passed"
ride_output="$(run_godot -s res://tests/ride_smoke.gd)"
echo "$ride_output" | grep "RIDE SMOKE TEST PASSED"
ui_output="$(run_godot -s res://tests/ui_smoke.gd)"
echo "$ui_output" | grep "UI SMOKE TEST PASSED"
workouts_output="$(run_godot -s res://tests/workouts_smoke.gd)"
echo "$workouts_output" | grep "WORKOUTS SMOKE TEST PASSED"
devices_output="$(run_godot -s res://tests/devices_smoke.gd)"
echo "$devices_output" | grep "DEVICES SMOKE TEST PASSED"
window_output="$(run_godot -s res://tests/window_smoke.gd)"
echo "$window_output" | grep "WINDOW SMOKE TEST PASSED"
settings_output="$(run_godot -s res://tests/settings_smoke.gd)"
echo "$settings_output" | grep "SETTINGS SMOKE TEST PASSED"
hud_output="$(run_godot -s res://tests/hud_smoke.gd)"
echo "$hud_output" | grep "HUD SMOKE TEST PASSED"
course_output="$(run_godot -s res://tests/course_smoke.gd)"
echo "$course_output" | grep "COURSE SMOKE TEST PASSED"
rider_output="$(run_godot -s res://tests/rider_smoke.gd)"
echo "$rider_output" | grep "RIDER SMOKE TEST PASSED"
analysis_output="$(run_godot -s res://tests/analysis_smoke.gd)"
echo "$analysis_output" | grep "ANALYSIS SMOKE TEST PASSED"
upload_output="$(run_godot -s res://tests/upload_smoke.gd)"
echo "$upload_output" | grep "UPLOAD SMOKE TEST PASSED"
error_output="$(run_godot -s res://tests/error_smoke.gd)"
echo "$error_output" | grep "ERROR SMOKE TEST PASSED"
