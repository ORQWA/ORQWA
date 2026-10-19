#!/usr/bin/env sh
# Runs a command inside the Torqa dev container (same image as the VS Code devcontainer),
# for use outside VS Code. Example: scripts/dev.sh scripts/check.sh
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

# From Git Bash on Windows: Docker wants the checkout as a Windows path, and the container
# paths must not be rewritten into Windows ones (MSYS turns `/workspaces/torqa` into
# `C:/Program Files/Git/workspaces/torqa` otherwise).
if command -v cygpath >/dev/null 2>&1; then
    root="$(cygpath -m "$root")"
    export MSYS_NO_PATHCONV=1
fi

docker build -q -t torqa-dev -f "$root/.devcontainer/Dockerfile" "$root" >/dev/null

# The commit for the About tab (R83): a worktree's Git directory lies outside the mounted
# checkout, so the build inside cannot ask Git itself.
commit="$(git -C "$root" rev-parse --short=7 HEAD 2>/dev/null || true)"

tty_flags=""
if [ -t 0 ] && [ -t 1 ]; then
    tty_flags="-it"
fi

. "$root/scripts/give-back-files.sh"

status=0
# shellcheck disable=SC2086 # tty_flags is intentionally empty or a single flag
docker run --rm $tty_flags \
    -e TORQA_COMMIT="$commit" \
    -v "$root:/workspaces/torqa" \
    -v torqa-cargo-registry:/usr/local/cargo/registry \
    -v torqa-target:/workspaces/torqa/core/target \
    -v torqa-cache:/root/.cache \
    -w /workspaces/torqa \
    torqa-dev "$@" || status=$?
give_back_files torqa-dev
exit "$status"
