---
name: torqa-physics
description: Change or tune how Torqa's rider moves — the ride simulation (speed from power, grade sent to the trainer) in torqa-physics, and the rider and bike motion in the 3D world (pitch, lean into bends, standing on the pedals, rock, sway, pedalling) — and check it end to end with tick traces and tests. Use for anything that changes how fast the rider goes or how rider and bike move on screen.
---

# Physics

Two layers, kept apart (CLAUDE.md: logic in Rust, Godot presents):

- **The ride simulation** — how fast the rider goes for their power, and the grade the
  trainer feels — is `core/torqa-physics` (R13–R15): `P·η / v = m·g·sin θ + m·g·Crr·cos θ
  + ½·ρ·CdA·v_air² + m·a`, tested in Rust. `lean_angle` (`tan λ = v² · κ / g`) lives there too
  and reaches Godot as `TorqaApp.lean_angle`. Anything that changes speed, distance, the FIT
  file or the trainer belongs here, never in GDScript.
- **Rider and bike motion** (R46, R88) is presentation only: it reads the ride state and never
  feeds back. `app/scenes/rider_motion.gd` (`RiderMotion`) turns grade, the bend's lean,
  effort (W/kg) and cadence into the bike's pitch, lean and rock and the rider's standing and
  sway; `app/scenes/rider_avatar.gd` (`RiderAvatar`) poses the model: cranks, wheels, the upper
  body about the seat and the hands, the legs by two-bone IK; `RideWorld._follow_ride` steps
  both every frame. The fit (hips, hands, bottom bracket, legs) comes from `art/riders`
  through `riders.json`.

Taken over from the author's earlier game project (choppa) and adapted: there the physics
drives a helicopter; the workflow and the lessons carry over.

## Rules

- **No scripted moves.** Motion follows from what the ride asks (grade, curve, effort,
  cadence) through springs, never keyframed sequences or timers that play a move.
- **Springs, critically damped.** `RiderMotion.spring` settles as fast as possible without
  overshoot; split long frames (`MAX_STEP`) so it stays stable at low frame rates. Overshoot
  on a grade step reads as the bike bucking.
- **Hysteresis for decisions.** Standing up needs the reason to last (`STAND_AFTER`), sitting
  down a clear drop below it for longer (`SIT_AFTER`): without, riders flicker at thresholds.
- **The legs must reach.** Any pose that moves the hips is limited so both legs reach their
  pedals all round the stroke (`_standing_reach`, the per-frame bisection in `animate`);
  `rider_smoke.gd` checks it. A leg stretched off its pedal is the first thing people see.
- **Hands on the hoods.** The upper body turns about the hands; moving it otherwise slides
  the hands along the bars — keep that small (`STAND_FORWARD`).
- Presentation constants are documented with their real-world meaning (degrees, cm, W/kg).

## Workflow

1. **Trace before tuning.** Write a throwaway SceneTree script in `app/tests/` (never commit
   it) that builds a `RiderAvatar` and a `RiderMotion`, steps them at 60 Hz through scenarios
   (flat, a step onto a climb at effort, easing off, a sprint, coasting into a bend) and
   prints per tick: standing, pitch, lean, rock, hip height, leg length against reach, crank
   angle. Run it headless:
   `godot --headless --path app -s res://tests/<probe>.gd` (run `--import` first after adding a
   class). Read the numbers; guessing constants wastes runs.
2. **Tests:** `rider_smoke.gd` (fit, IK, standing, coasting, the motion over a ride);
   `cargo test -p torqa-physics` for the simulation. Then `scripts/check.sh`.
3. **Look at it:** `scripts/render-riders.sh` (views `standing`, `standing-chase`, `lean`), and a
   chase view on a steep climb with the fake trainer at high power (the `tmp_road.gd` template
   of `torqa-render-review` with its power raised). On this PC render natively on the GPU
   (see that skill). Ask the owner to ride it: nobody else can judge the feel.

## Lessons (rider motion, 2026-10-18)

- Raising the hips by turning the body about the hands alone moves them up, hardly forward:
  the leg at the bottom of the stroke runs out of reach after ~3 cm (the saddle is set for a
  20–40° knee bend). Standing riders move the hips forward over the cranks and point the toes
  down; both buy reach.
- Limiting the stand per frame to what the legs reach makes the body bob 14 cm with every
  stroke. Precompute the reach over the whole stroke and stand a little beyond it
  (`STAND_BOB`): the legs then trim only near the bottom, a bob of ~2 cm.
- Halving a tilt until the legs reach makes it jump between steps; bisect instead.
- First-order smoothing (lerp) of pitch and lean starts abruptly; a second-order spring
  starts and ends softly.
