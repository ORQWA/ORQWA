#!/usr/bin/env sh
# Called only after the tagged commit's checks and every platform build succeed.
set -eu

: "${RELEASE_TAG:?Set RELEASE_TAG to an existing tag}"
: "${GH_REPO:?Set GH_REPO to the release repository}"

set -- \
    dist/ORQWA-macos-arm64.zip \
    dist/orqwa-cli-macos-arm64.tar.gz \
    dist/ORQWA-windows-x86_64.zip \
    dist/ORQWA-windows-x86_64.msi \
    dist/orqwa-cli-windows-x86_64.zip \
    dist/ORQWA-linux-x86_64.tar.gz \
    dist/orqwa-cli-linux-x86_64.tar.gz \
    dist/ORQWA-linux-arm64.tar.gz \
    dist/orqwa-cli-linux-arm64.tar.gz \
    dist/ORQWA-android-arm64.apk

# A missing upload must not turn a successful build into an incomplete release.
for asset do
    if [ ! -s "$asset" ]; then
        echo "Missing or empty release asset: $asset" >&2
        exit 1
    fi
done

# Torqa 0.5.0 installs an update only from an installer under its old name (ADR 0026): the same
# MSI goes up under that name too, until riders have moved past it.
cp dist/ORQWA-windows-x86_64.msi dist/Torqa-windows-x86_64.msi
set -- "$@" dist/Torqa-windows-x86_64.msi

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
