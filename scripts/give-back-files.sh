# Sourced by dev.sh and art.sh. With rootful Docker on Linux (e.g. a WSL clone) the containers
# run as root and leave what they write in the checkout owned by root, which the user's own
# tools then cannot change: Windows Godot rendering through \\wsl.localhost fails to copy the
# extension DLL in app/bin/. Docker Desktop, rootless Docker and Podman map the container's root
# to the user, so the host sees no root-owned files there and this does nothing.

# Usage: give_back_files [docker run options] <image> — after the container, $root the checkout.
give_back_files() {
    [ "$(id -u)" -ne 0 ] || return 0
    [ -n "$(find "$root" -uid 0 -print -quit 2>/dev/null)" ] || return 0
    docker run --rm -v "$root:/workspaces/orqwa" --entrypoint find "$@" \
        /workspaces/orqwa -uid 0 -exec chown "$(id -u):$(id -g)" {} +
}
