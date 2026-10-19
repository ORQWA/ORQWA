#!/usr/bin/env sh
# Runs a command in the Torqa art container (headless Blender, ADR 0009). Example, to rebuild
# the building models: scripts/art.sh blender --background --factory-startup --python art/buildings/build.py
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

# From Git Bash on Windows, as in dev.sh: Docker wants a Windows path, and the container paths
# must not be rewritten into Windows ones.
if command -v cygpath >/dev/null 2>&1; then
    root="$(cygpath -m "$root")"
    export MSYS_NO_PATHCONV=1
fi

docker build -q --platform linux/amd64 -t torqa-art "$root/art" >/dev/null

. "$root/scripts/give-back-files.sh"

status=0
docker run --rm --platform linux/amd64 \
    -v "$root:/workspaces/torqa" \
    -w /workspaces/torqa \
    torqa-art "$@" || status=$?
give_back_files --platform linux/amd64 torqa-art
exit "$status"
