# ADR 0021 — The Windows extension cross-built in the dev container, for native renders

- Status: accepted
- Date: 2026-10-19

## Context

The dev container renders the 3D world with lavapipe, Vulkan on the CPU: correct, but a town or
a mountain pass takes minutes a frame, and a heavy route can run the container out of memory.
On a Windows PC with a discrete GPU, the portable Godot editor renders the same views in
seconds — but it loads the GDExtension as a Windows DLL, and until now only the `windows` CI
job built one. CI runs on pull requests and `main`, so every look at a change in the world's
Rust code meant pushing it as a pull request and waiting about 20 minutes for the job.

GPU passthrough does not help: Docker Desktop on Windows hands containers the GPU for CUDA, but
no Vulkan driver for it, so Godot in the container still falls back to lavapipe.

Options considered:

- **Keep CI as the only source**: no new tooling, but slow round trips for every visual change.
- **A Rust toolchain on the Windows host**: fast, against the containers-only rule.
- **Cross-compile the DLL in the dev container** with MinGW for Rust's `x86_64-pc-windows-gnu`
  target, FFmpeg cross-built for Windows the same way.

## Decision

- The dev image carries the MinGW cross compiler (`gcc-mingw-w64-x86-64`), Rust's
  `x86_64-pc-windows-gnu` target and FFmpeg built for Windows by `scripts/build-ffmpeg.sh`
  (`/opt/ffmpeg-windows`, the same pinned release and parts as everywhere else).
- `scripts/build-gdext.sh [debug|release] windows` builds `orqwa_gd.dll` in the container,
  linked statically (it needs only Windows' own DLLs), strips it and puts it in `app/bin/`,
  where the portable Godot editor on the host loads it for native renders.
- It is for development only: the released Windows app and its DLL still come from the
  `windows` CI job (MSVC), which nothing here changes.

## Consequences

- A native render of a Rust change is minutes away instead of a CI round trip; the container
  rule holds (nothing is installed on the host beyond the portable editor).
- The dev image grows by the MinGW toolchain and a second FFmpeg build; the FFmpeg layer takes
  longer to build the first time.
- The development DLL is built with a different toolchain (MinGW) than the released one
  (MSVC): a problem only one of them shows is possible, so the CI build stays what is tested
  before a release.
