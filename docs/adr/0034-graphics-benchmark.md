# ADR 0034 — Graphics calibration from rendered frames

- Status: accepted
- Date: 2026-10-26

## Context

Choosing a graphics preset by hand requires trial rides (#353, R150). Device names cannot
reliably predict performance across GPUs, rendering backends, screen resolutions and power
modes. A calibration must work before a rider imports a course, without network requests,
and preserve their course, devices, ride history and settings on cancellation.

## Decision

Settings → Graphics offers **Benchmark graphics**. A separate viewport draws a deterministic
offline road, village, dense forest, grass and water using `RideWorld`, the production mesh
conversion, instance selection, model loaders, shaders and quality presets. Its 3D target is
the main window's current render resolution. It owns its world and environment; the loaded
course is never replaced. The fixture is built in Rust and adds no downloads or assets.

Every preset from Low to Ultra has three seconds of shader warm-up followed by three seconds
of frame samples. Uploads and background ground simplification finish before timing starts.
Wall time is measured between `RenderingServer.frame_post_draw` signals, so it includes CPU
and rendering costs rather than a smoothed FPS counter. A repeating camera pan covers the
same views for each preset. The frame cap and V-Sync are temporarily disabled.

The Rust application layer chooses the highest preset whose 95th-percentile frame time is
within the 60 fps budget with 5% scheduling tolerance. Repeated stutters therefore fail even
when average FPS is high. If none meets the target, Low is selected and the rider is told it
was below target. Only a complete run can yield a result. Applying it atomically saves the
preset and clears every detail override in `computer.toml`, including the frame cap and
V-Sync overrides, so the next ride uses what was measured with the usual 60 fps cap.

Cancel, Escape, closing the preview, application focus loss, resizing the main window or a
two-minute timeout discards the run. Completion and cancellation restore the previous live
frame cap, V-Sync and global shadow-atlas size before closing the preview. Headless rendering
cannot calibrate hardware. No ride is started, recorded or uploaded.

## Consequences

The result is a starting preset for this resolution and power mode, not a promise that
every course holds 60 fps: dense cities, weather and thermal throttling can differ from the
bounded workload. Riders can rerun after changing display or power mode, and choose manually.
No generator or stored-world version changes, dependencies or network endpoints are needed.

Rust tests cover warm-up, preset selection, stutters, incomplete and invalid samples and
atomic settings updates. The Godot smoke test covers the binding, application and cancelled
settings, and can exercise the actual preview under software Vulkan. Software-renderer FPS
is not evidence for native GPU performance; native runs remain the way to check calibration
on macOS, Windows, Linux and Android hardware.
