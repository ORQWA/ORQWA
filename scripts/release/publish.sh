#!/usr/bin/env sh
# Called only after the tagged commit's checks and every platform build succeed.
set -eu

: "${RELEASE_TAG:?Set RELEASE_TAG to an existing tag}"
: "${GH_REPO:?Set GH_REPO to the release repository}"

# The builds, as the app's name and the platform, each published with the version between them
# (ORQWA-0.5.2-windows-x86_64.msi), so downloads of different releases stay apart (#322).
built="
    ORQWA macos-arm64.zip
    orqwa-cli macos-arm64.tar.gz
    ORQWA windows-x86_64.zip
    ORQWA windows-x86_64.msi
    orqwa-cli windows-x86_64.zip
    ORQWA linux-x86_64.tar.gz
    orqwa-cli linux-x86_64.tar.gz
    ORQWA linux-arm64.tar.gz
    orqwa-cli linux-arm64.tar.gz
    ORQWA android-arm64.apk
"
version="${RELEASE_TAG##*/}"
version="${version#v}"

# A missing upload must not turn a successful build into an incomplete release.
while read -r app platform; do
    [ -n "$app" ] || continue
    if [ ! -s "dist/$app-$platform" ]; then
        echo "Missing or empty release asset: dist/$app-$platform" >&2
        exit 1
    fi
done <<EOF_BUILT
$built
EOF_BUILT

set --
while read -r app platform; do
    [ -n "$app" ] || continue
    cp "dist/$app-$platform" "dist/$app-$version-$platform"
    set -- "$@" "dist/$app-$version-$platform"
done <<EOF_BUILT
$built
EOF_BUILT

# Every ORQWA released before the versioned names installs an update only from the installer
# under its plain name, and Torqa 0.5.0 only under its old one (ADR 0026): the installers go up
# under those names too, until riders have moved past them.
cp dist/ORQWA-windows-x86_64.msi dist/Torqa-windows-x86_64.msi
set -- "$@" dist/ORQWA-windows-x86_64.msi dist/ORQWA-android-arm64.apk \
    dist/Torqa-windows-x86_64.msi

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
