#!/usr/bin/env sh
# Git operations stay on the host so the user's credential helper remains available.
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
# An existing checkout may contain an agent's work. Never reset it to the recorded pointer.
if [ ! -e wiki/.git ]; then
    git submodule update --init -- wiki
fi
git -C wiki rev-parse --show-toplevel >/dev/null
