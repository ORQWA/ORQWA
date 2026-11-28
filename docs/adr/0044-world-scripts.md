# ADR 0044 — World scripts, group rides on worlds and bundled workouts

- Status: proposed
- Date: 2026-11-28
- Amends: [ADR 0042](0042-world-files.md), [ADR 0043](0043-signed-licensed-worlds.md), R285
- Requirements: R302–R305

## Context

A world file ([ADR 0042](0042-world-files.md)) holds a static place and the routes through it.
Riding it works like any course: each rider moves by ORQWA's physics, on ORQWA's rider and bike,
with ORQWA's HUD. The maintainers want some worlds to be more than a place: worlds with rules
of their own, made by world authors, released on their own schedule. If every such world needed
a change to ORQWA first, each one would wait for a release, and ORQWA would collect features
only one world uses.

What such worlds need goes beyond a static file:

- Their own figures and moving objects, animated, in place of or next to the rider.
- Their own rules for how riders move: alone, together, held, paced.
- Their own trainer targets at points of the ride, within what is safe.
- Their own information on screen, and their own results recorded with the ride.
- Riding them as a group, when the world allows it. Today world routes cannot be chosen for
  group rides (R285).
- Workouts made for them.

The maintainers have decided that worlds bring their behaviour **without ORQWA changing for
each world**, and that a world decides, in its signed file, whether it may be ridden as a group.

Options considered:

- **More declarative extensions**, one per feature. Safe and simple, but every new kind of
  behaviour is an ORQWA change, which is what the maintainers want to avoid.
- **A scripting language embedded in ORQWA** (Lua, JavaScript). Flexible, but these runtimes
  have broad standard libraries to strip, are hard to meter, and tie world authors to one
  language.
- **Native plugins.** Full access to the machine. Ruled out.
- **WebAssembly with a small, fixed host API.** A module can call only the functions the host
  gives it, can be metered and limited in memory, and can be written in any language that
  compiles to it. Runtimes:
  - `wasmtime`, `wasmer`: fast, but they compile machine code at run time (not allowed on some
    platforms) and are large.
  - **`wasmi`**: a pure-Rust interpreter, no JIT, with fuel metering. It builds wherever
    ORQWA's core builds, including Android. A world script does little work per tick, so an
    interpreter is fast enough.

## Decision

Three optional world extensions are added to format 1, as [ADR 0042](0042-world-files.md)
allows. Their exact shape is in [world-files.md](../world-files.md#extensions).

| Extension | What it adds |
|---|---|
| `ORQWA_script` | A WebAssembly module that runs during the ride, its configuration, and the models it may place. |
| `ORQWA_group` | Whether the world may be ridden as a group, and by how many riders. |
| `ORQWA_workouts` | Workout files that come with the world. |

A world lists in `extensions_required` those it cannot be ridden without. An ORQWA that does
not know them asks for an update, as for any required extension.

### The script's place

- **ORQWA keeps doing the work.** The physics, the trainer, drawing the world and the HUD stay
  in ORQWA. A script only decides, each tick, from what it is told: it sets parameters and
  requests, and ORQWA carries them out within its limits.
- **Generic primitives only.** The API knows no particular world or game. It speaks of bodies
  moving along a route, trainer requests, nodes placed in the scene, screen elements and
  results. Any behaviour a world needs is expressed in them.
- **Defaults are today's ride.** What a script does not set behaves as a ride on a world
  without a script. A world whose script stops working is still ridden, as a plain world.

### The interface

- The module exports `alloc` and `tick`. Each tick, ORQWA writes a JSON document of the ride's
  state into the module's memory and calls `tick`; the module returns a JSON document of its
  outputs. The only function ORQWA gives the module is `log`, in the import module `orqwa_v1`.
- **In**: the time since the last tick and the ride's time; the host's API version and the
  features it offers; the world's configuration for the script; the route's shape; each
  rider's slot, name, power, cadence, mass, FTP and connection, and the body they move with;
  each body's distance and speed; the riders' presses of a generic **world action** input; and
  the state the script returned on the previous tick.
- **Out**, six primitives:
  - **Bodies**: what moves along a route. A body is either **driven** (ORQWA's physics, from
    the summed power of the riders it carries, with added mass and factors on rolling and air
    resistance) or **scripted** (the script sets its speed). Each rider moves with one body. By
    default each rider is a driven body of their own, as today.
  - **Trainer**: per rider, ORQWA's own control vocabulary: the road (default), a grade, a
    power or a resistance.
  - **Scene**: nodes that show the world's models, or ORQWA's rider, anchored to a body, a
    rider or a place on the route, with an offset, rotation, scale and visibility, and playing
    the model's own animation clips at a given phase or rate; lines between nodes; whether each
    rider's ORQWA rider is shown; and a camera hint the rider's own camera choice can override.
  - **Screen**: panels of text, values with units, bars, grids and images, placed in ORQWA's
    HUD areas and drawn in ORQWA's style, for one rider or all. A world supplies content, not
    layout.
  - **Results**: values and timed marks, saved with the activity.
  - **State**: an opaque document handed back on the next tick.
- Every quantity is in SI units.

### Safety

- **No access beyond the ride.** The module can reach only what ORQWA hands it. It has no
  files, network, clock, devices, processes or operating system, and no data from other worlds
  or from the rider's profile beyond FTP, mass, name and slot. **The API will never offer
  these.** When a world is opened, a module that imports anything outside the host's API, or
  that uses WebAssembly features outside the accepted set, is refused.
- **Limits.** Each tick has a fuel budget; memory and the size of input, output and state are
  capped. A module that traps, runs out of fuel or returns output ORQWA cannot read is stopped
  for the rest of the ride. The ride goes on as a plain world ride, and ORQWA says so.
- **ORQWA clamps what a script asks of a trainer**, on each rider's own device: grades within
  the limits ORQWA already sends, scaled by the rider's difficulty setting; power no higher
  than twice the rider's FTP or what the trainer can do; changes no more often than ORQWA
  already sends them. Pausing and stopping always win. Body parameters are clamped too.
- **Only signed worlds run scripts.** A script is part of a signed world
  ([ADR 0043](0043-signed-licensed-worlds.md)), so it cannot be changed or added after
  signing, and withdrawing a key or world version withdraws its script.

### Backwards compatibility

World files stay working for good, and so do their scripts. These rules hold for every later
ORQWA:

1. **The interface is frozen.** `alloc`, `tick` and `orqwa_v1` never change. New host
   functions come only in further import modules (`orqwa_v2`, …); earlier ones stay.
2. **Every API version is kept.** A world names the lowest API version its script needs. ORQWA
   runs every version up to its own; a newer one asks for an update.
3. **Documents only grow.** New fields are optional, with defaults that keep earlier
   behaviour. No field is removed or renamed, or changes its type, unit or meaning. ORQWA
   ignores output fields it does not know, and scripts are expected to ignore input fields
   they do not know.
4. **Unknown values degrade safely.** Any element may name a fallback. Without one, an unknown
   kind of body leaves the rider on their own body, an unknown trainer request becomes the
   road, and an unknown node or screen element is left out.
5. **Features are detected, not guessed.** The input lists the features the host offers.
6. **No fixed tick rate.** Scripts use the time they are given; ORQWA may tick more or less
   often.
7. **The accepted WebAssembly features only grow.** A runtime update must keep accepting every
   module an earlier ORQWA accepted.
8. **Limits change only for safety,** and each change is recorded in this ADR.
9. **Nothing is removed.** Parts of the API may be described as superseded; they keep working.

Tests hold ORQWA to this: a set of frozen script modules with recorded inputs and outputs that
every later build must reproduce, a snapshot of the documents' schemas that fails on any change
other than an addition, and tests of every fallback.

### Group rides on worlds

- R285 is amended: a world's routes may be chosen for a group ride when its signed
  `ORQWA_group` allows it, for at most the riders it names (never more than ORQWA's group
  limit). Every rider needs the same world: the same id, version and manifest. ORQWA still
  offers no export of world files; each rider has their own copy.
- The **host runs the script**, for everyone. Its outputs reach the other riders in one
  generic, versioned group message, and their world actions reach the host the same way. The
  others show what the host sends and apply its trainer requests **through their own clamps**,
  so a host cannot ask more of another rider's trainer than that rider's own ORQWA would.
- A world without `ORQWA_group` cannot be ridden as a group, as before.

### Bundled workouts

`ORQWA_workouts` names workout files in the world, in formats ORQWA already reads. The library
offers them with the world's routes. When a structured workout runs, its targets are what the
trainer holds; a script's trainer requests then give way, and the script still drives the
scene, the screen and the results.

## Consequences

- World authors can release worlds with their own behaviour without waiting for ORQWA. ORQWA
  changes only when a world needs something the API does not offer, and that change then serves
  every world.
- `wasmi` becomes a dependency, subject to approval under the dependency rules.
- The API is a promise kept for good. It is kept small, and every addition is made with that in
  mind.
- Two crates are added: one for the documents' types, which scripts may use as well, and one
  for the runtime. `orqwa-cli` gains a command that runs a world's script headless with
  simulated riders, for world authors.
- Models a script places may have skins and animations. The rest of a world's glTF stays
  static, as ADR 0042 sets out.
- A script that misbehaves costs its world its behaviour, never the ride or the rider's safety.
