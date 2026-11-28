#!/usr/bin/env sh
# Runs a command inside the ORQWA dev container (same image as the VS Code devcontainer),
# for use outside VS Code. Example: scripts/dev.sh scripts/check.sh
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"

# From Git Bash on Windows: Docker wants the checkout as a Windows path, and the container
# paths must not be rewritten into Windows ones (MSYS turns `/workspaces/orqwa` into
# `C:/Program Files/Git/workspaces/orqwa` otherwise).
if command -v cygpath >/dev/null 2>&1; then
    root="$(cygpath -m "$root")"
    export MSYS_NO_PATHCONV=1
fi

docker build -q -t orqwa-dev -f "$root/.devcontainer/Dockerfile" "$root" >/dev/null

# The branch and commit for the About tab (R83, R130): a worktree's Git directory lies outside
# the mounted checkout, so the build inside cannot ask Git itself.
commit="$(git -C "$root" rev-parse --short=7 HEAD 2>/dev/null || true)"
branch="$(git -C "$root" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
# Linked worktrees point outside the mounted checkout. Wiki/link checks need to read that
# index too; expose only the shared Git metadata, never write it from container tooling.
git_common_dir="$(git -C "$root" rev-parse --path-format=absolute --git-common-dir)"
if command -v cygpath >/dev/null 2>&1; then
    git_common_dir="$(cygpath -m "$git_common_dir")"
fi

tty_flags=""
if [ -t 0 ] && [ -t 1 ]; then
    tty_flags="-it"
fi

# The host's GPU and display, for hardware Vulkan: a render that takes minutes per frame on
# software Vulkan takes seconds on a real one. A Vulkan surface needs the host's display,
# because the virtual X server has no DRI3. ORQWA_NO_GPU=1 keeps both out, for a GPU the
# container's drivers cannot use.
gpu_flags=""
if [ "${ORQWA_NO_GPU:-0}" != "1" ] && [ -d /dev/dri ]; then
    gpu_flags="--device /dev/dri"
    # Rootless Podman drops the caller's supplementary groups, and the render node is
    # group-readable; keeping them lets the container open it as the host user does.
    if docker info --format '{{.Host.Security.Rootless}}' 2>/dev/null | grep -qx true; then
        gpu_flags="$gpu_flags --group-add keep-groups"
    fi
    if [ -n "${DISPLAY:-}" ] && [ -d /tmp/.X11-unix ]; then
        gpu_flags="$gpu_flags -e DISPLAY=$DISPLAY -v /tmp/.X11-unix:/tmp/.X11-unix"
        if [ -n "${XAUTHORITY:-}" ] && [ -r "$XAUTHORITY" ]; then
            gpu_flags="$gpu_flags -e XAUTHORITY=/run/host/xauthority"
            gpu_flags="$gpu_flags -v $XAUTHORITY:/run/host/xauthority:ro"
        fi
    fi
fi

. "$root/scripts/give-back-files.sh"

status=0
# shellcheck disable=SC2086 # tty_flags and gpu_flags are intentionally empty or single flags
docker run --rm $tty_flags $gpu_flags \
    -e ORQWA_COMMIT="$commit" \
    -e ORQWA_BRANCH="$branch" \
    -v "$root:/workspaces/orqwa" \
    -v "$git_common_dir:$git_common_dir:ro" \
    -v orqwa-cargo-registry:/usr/local/cargo/registry \
    -v orqwa-target:/workspaces/orqwa/core/target \
    -v orqwa-cache:/root/.cache \
    -w /workspaces/orqwa \
    orqwa-dev "$@" || status=$?
give_back_files orqwa-dev
exit "$status"
