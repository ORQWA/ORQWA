#!/usr/bin/env sh
# Writes the licence notices of the Rust crates built into the app, which its About tab shows
# (R83, ADR 0018), to core/torqa-app/notices/crates.txt. Run it after changing dependencies;
# with --check, it fails if the file is out of date instead. Runs inside the dev container.
# Usage: scripts/notices.sh [--check]
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
notices="$root/core/torqa-app/notices/crates.txt"
generated="$(mktemp)"
trap 'rm -f "$generated"' EXIT

# The extension's dependencies, not the workspace's: the CLI's and the tests' stay out.
cargo about generate --fail --locked \
    -c "$root/core/about.toml" -m "$root/core/torqa-gd/Cargo.toml" \
    -o "$generated" "$root/core/about.hbs"

case "${1:-}" in
    --check)
        if ! cmp -s "$generated" "$notices"; then
            echo "core/torqa-app/notices/crates.txt is out of date: run scripts/notices.sh" >&2
            exit 1
        fi
        ;;
    "") cat "$generated" >"$notices" ;;
    *) echo "usage: $0 [--check]" >&2; exit 1 ;;
esac
