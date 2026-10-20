#!/usr/bin/env sh
# Called only after the tagged commit's checks and every platform build succeed.
set -eu

: "${RELEASE_TAG:?Set RELEASE_TAG to an existing tag}"
: "${GH_REPO:?Set GH_REPO to the release repository}"

set -- \
    dist/Torqa-macos-arm64.zip \
    dist/torqa-cli-macos-arm64.tar.gz \
    dist/Torqa-windows-x86_64.zip \
    dist/Torqa-windows-x86_64.msi \
    dist/torqa-cli-windows-x86_64.zip \
    dist/Torqa-linux-x86_64.tar.gz \
    dist/torqa-cli-linux-x86_64.tar.gz \
    dist/Torqa-linux-arm64.tar.gz \
    dist/torqa-cli-linux-arm64.tar.gz \
    dist/Torqa-android-arm64.apk

# A missing upload must not turn a successful build into an incomplete release.
for asset do
    if [ ! -s "$asset" ]; then
        echo "Missing or empty release asset: $asset" >&2
        exit 1
    fi
done

if draft="$(gh release view "$RELEASE_TAG" --json isDraft --jq .isDraft)"; then
    if [ "$draft" = false ]; then
        echo "Release $RELEASE_TAG is already published; keeping its assets."
        exit 0
    fi
else
    gh release create "$RELEASE_TAG" --verify-tag --generate-notes --draft \
        --title "$RELEASE_TAG"
fi

# Upload failures leave a draft. Rerunning the failed job completes that same draft.
gh release upload "$RELEASE_TAG" "$@" --clobber
gh release edit "$RELEASE_TAG" --draft=false
