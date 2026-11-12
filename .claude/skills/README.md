# Skills

Agent skills for working on ORQWA (Claude Code loads them from here when a task matches).

## Roles and hygiene

Who a session is and what it hands to whom (see CLAUDE.md "Skills"):

- `developer` — build and fix in Rust (`core/`) and Godot presentation (`app/`), test, PR to
  `main`; the working rules and the hand-off with the reviewer.
- `reviewer` — read, run, report; a ranked, copyable report, no fixes. Wraps `/code-review`.
- `producer` — keep `docs/PLAN.md`, `docs/REQUIREMENTS.md`, `docs/OPEN-QUESTIONS.md` and the
  issues telling one story; triage, standup. No code.
- `tester` — exercise the build (fake trainer, screenshots, the owner's real hardware); turn
  breaks into a failing test and an issue. No fixes.
- `operator` — releases through GitHub Actions and CI health; ORQWA is offline-first, so there
  is no live service.
- `security` — hold the offline-first, privacy promise; audit what the app sends and stores.
  Wraps `/security-review`.
- `artist` — own the look to one standard; judge every visual change on a render. The how lives
  in the domain skills below.
- `ship` — finish any change: full checks, docs + wiki, images, a no-attribution commit, a PR
  off `main`, CI.

## Domain skills

ORQWA's own:

- `orqwa-look` — the art direction: as realistic as possible (ADR 0020), replacing the
  faceted pastel look.
- `orqwa-art-pipeline` — making models and textures with headless Blender in the art
  container, with quick Cycles previews. Its texture baker, previews and bpy pitfalls come from
  the author's earlier game project (choppa).
- `orqwa-render-review` — rendering the world and models in the dev container for review.
- `orqwa-physics` — the ride simulation and the rider and bike motion: where they live, the
  rules (springs, hysteresis, legs that reach), tick traces before tuning. Adapted from the
  author's earlier game project (choppa).

Imported unchanged from [ra100/blender-claude-plugin](https://github.com/ra100/blender-claude-plugin)
at commit `78e9151`, MIT licence (`LICENSE` in each folder), for Blender 5.x. Their "MCP-first"
advice does not apply here; ORQWA runs Blender headless (see `orqwa-art-pipeline`):

- `blender-python-scripting`, `blender-modeling-modifiers`, `blender-animation-rigging`,
  `blender-scene-rendering`, `blender-geometry-nodes`.

To update them, copy the folders again from upstream and change the commit above.
