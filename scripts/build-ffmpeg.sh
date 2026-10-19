#!/usr/bin/env sh
# Builds FFmpeg's libraries for torqa-video (ADR 0010): a pinned release, its download verified,
# static, with only the parts used (demuxing, decoding, scaling), installed under the given
# prefix for ffmpeg-sys-next to find through FFMPEG_DIR. Native builds, or for Windows from
# Linux with MinGW (`windows`, for the dev image's Windows extension, ADR 0021), or for 64-bit
# ARM Android with the NDK in $ANDROID_NDK_ROOT (`android`, for the Android image, ADR 0024); on
# Windows itself run it from MSYS2's sh with MSVC's cl and link on PATH.
# Usage: scripts/build-ffmpeg.sh <prefix> [windows|android]
set -eu

version="9.0.2"
# Of https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.gz; FFmpeg publishes PGP signatures rather
# than checksums, so this is the download's own, taken 2026-10-08.
sha256="84960df915059e8754fef2cd7c9afeb614062b1b5458ec471eecee619ee04e98"
prefix="${1:?usage: $0 <prefix> [windows|android]}"
target="${2:-native}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM
archive="$work/ffmpeg-$version.tar.gz"
curl -fsSL -o "$archive" "https://ffmpeg.org/releases/ffmpeg-$version.tar.gz"
if command -v sha256sum >/dev/null 2>&1; then
    echo "$sha256  $archive" | sha256sum -c -
else
    echo "$sha256  $archive" | shasum -a 256 -c -
fi
tar -xzf "$archive" -C "$work"
cd "$work/ffmpeg-$version"

# What ffmpeg-sys-next's own build configured, minus tuning for the build machine: nothing
# autodetected (so no stray system dependencies), no programs, docs or network, static and
# position independent (the extension is a shared library), only avcodec, avformat, avutil and
# swscale, LGPL only. MSVC brings its own threads; the others use pthreads.
set -- \
    --prefix="$prefix" \
    --disable-autodetect --disable-programs --disable-doc --disable-network \
    --disable-debug --enable-stripping \
    --enable-static --disable-shared --enable-pic \
    --disable-avdevice --disable-avfilter --disable-swresample \
    --enable-avcodec --enable-avformat --enable-swscale
if [ "$target" = windows ]; then
    # Windows' own threads, as the MSVC build uses.
    set -- "$@" --enable-cross-compile --target-os=mingw32 --arch=x86_64 \
        --cross-prefix=x86_64-w64-mingw32- --enable-w32threads
elif [ "$target" = android ]; then
    # The NDK's clang for Android 7 (API 24), the oldest Godot's Android build supports.
    ndk="${ANDROID_NDK_ROOT:?set ANDROID_NDK_ROOT to the NDK}/toolchains/llvm/prebuilt/linux-x86_64/bin"
    set -- "$@" --enable-cross-compile --target-os=android --arch=aarch64 --cpu=armv8-a \
        --cc="$ndk/aarch64-linux-android24-clang" --cxx="$ndk/aarch64-linux-android24-clang++" \
        --ar="$ndk/llvm-ar" --ranlib="$ndk/llvm-ranlib" --nm="$ndk/llvm-nm" \
        --strip="$ndk/llvm-strip" --enable-pthreads
else
    case "$(uname -s)" in
        MINGW* | MSYS* | CYGWIN*) set -- "$@" --toolchain=msvc ;;
        *) set -- "$@" --enable-pthreads ;;
    esac
fi
sh ./configure "$@" || { tail -n 80 ffbuild/config.log; exit 1; }
jobs="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 2)"
make -j"$jobs"
make install
echo "FFmpeg $version installed under $prefix"
