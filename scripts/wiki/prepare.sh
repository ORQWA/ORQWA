#!/usr/bin/env sh
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
branch="${1:-}"
if [ -z "$branch" ]; then
    echo 'Usage: make wiki-prepare BRANCH=docs/your-change' >&2
    exit 1
fi
git check-ref-format --branch "$branch" >/dev/null
sh scripts/wiki/init.sh
git -C wiki fetch origin
published="$(git -C wiki ls-remote --symref origin HEAD | sed -n 's|^ref: refs/heads/\([^[:space:]]*\).*|\1|p')"
if [ -z "$published" ] || [ "$branch" = "$published" ]; then
    echo 'Choose a preparation branch different from the published wiki branch.' >&2
    exit 1
fi
current="$(git -C wiki symbolic-ref --quiet --short HEAD || true)"
if [ "$current" != "$branch" ]; then
    if git -C wiki show-ref --verify --quiet "refs/heads/$branch"; then
        git -C wiki switch "$branch"
    elif git -C wiki show-ref --verify --quiet "refs/remotes/origin/$branch"; then
        git -C wiki switch --track -c "$branch" "origin/$branch"
    else
        git -C wiki switch -c "$branch"
    fi
fi
# Refuse divergent remote history instead of discarding either side's work.
if git -C wiki show-ref --verify --quiet "refs/remotes/origin/$branch"; then
    if ! git -C wiki merge-base --is-ancestor "origin/$branch" HEAD; then
        echo 'Wiki branch is behind or divergent. Reconcile it before preparing.' >&2
        exit 1
    fi
fi
make wiki-check
git -C wiki add --all
if ! git -C wiki diff --cached --quiet; then
    git -C wiki commit -m "docs: update usage wiki ($branch)"
fi
git -C wiki push --set-upstream origin "HEAD:refs/heads/$branch"
echo 'Wiki changes prepared; published pages have not changed.'
echo 'Stage the application pointer explicitly: git add wiki'
published_revision="$(git -C wiki rev-parse "origin/$published")"
prepared_revision="$(git -C wiki rev-parse HEAD)"
# The branch went to the push remote: a contributor's fork wiki when they cannot push to
# the published wiki (see docs/development.md), else the published wiki itself.
pushed_to="$(git -C wiki remote get-url --push origin | sed -E 's#^(git@github\.com:|https://github\.com/)##; s#\.wiki(\.git)?$##; s#\.git$##')"
echo "Review: https://github.com/$pushed_to/wiki/_compare/$published_revision...$prepared_revision"
if [ "$pushed_to" != "Torqa-Indoor-Cycling/Torqa" ]; then
    echo "The branch is on $pushed_to's wiki: ask a maintainer to fetch it into the published wiki before merging,"
    echo "  git -C wiki fetch https://github.com/$pushed_to.wiki.git $branch && git -C wiki push origin FETCH_HEAD:refs/heads/$branch"
fi
