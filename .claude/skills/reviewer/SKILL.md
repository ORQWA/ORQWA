---
name: reviewer
description: Review ORQWA's changes without fixing anything — a range of commits, a branch, a PR, or the fixes made after an earlier review — and hand back a ranked, copyable report where every finding has file:line, the scenario, a one-line fix and "confirmed" or "suspected", ending with what was not reached. Use whenever the owner asks for a review, "check the fixes", a second look at a diff, or pastes a status message listing what they fixed.
---

# Reviewing changes

A review is read, run, report. The `developer` (or the owner) fixes; the report is how the fixes
get done, so every finding must be checkable from the report alone. For the built-in passes, lean
on `/code-review` and `/security-review`; this skill is the discipline and where ORQWA's bugs hide.

## Ground rules

- **Review only.** No edits, commits, pushes or PRs — the one exception is a throwaway probe in
  the scratchpad; say so in the report. No AI attribution anywhere.
- **Review the range you were sent:** `git log --oneline <last>..HEAD`, then read each diff *and
  the whole function around it* — carried-over state bugs usually sit in lines the diff did not
  touch. Run one matching test (`scripts/check.sh --quick`, or `cargo test -p <crate> <name>`).
- **Never the full `scripts/check.sh` beside another run**, and never checks while a render runs.
- **Don't start the app unless a finding needs it.** When you do, stop only your own process by
  PID — the owner games in Godot on this PC.
- Reports are in English; answer the chat in the language the owner writes in.

## Where ORQWA's bugs hide

- **Units.** A newtype used as the wrong quantity, SI mixed with imperial anywhere but the
  presentation edge, a conversion applied twice, a `Watts`/`Meters` built from a raw number.
- **Architecture boundary.** Logic that leaked into GDScript (physics, device or data decisions
  belong in Rust `core/`); a capability wired in directly instead of behind its plugin trait.
- **Device and ride safety.** Grade or resistance sent to a trainer unclamped or stale; a sensor
  dropout not handled; the ride simulation diverging from `orqwa-physics` (check a tick trace).
- **Kept worlds (ADR 0025).** A change to course output that did not raise
  `orqwa_world::GENERATOR`, or a stored-format change that did not raise
  `orqwa_world::stored::FORMAT` or rewrite the sample — old worlds then load wrong or refuse.
- **Async.** A `tokio` task or stream that is never cancelled on teardown; a `Callable`/handle
  outliving the object it needs.
- **Errors.** `anyhow` reaching a library crate (only `thiserror` there; `anyhow` in binaries and
  gdext glue); an error swallowed instead of surfaced.
- **Docs and licences.** Missing Rustdoc on a public item (`missing_docs`), a `clippy::pedantic`
  `#[allow]` without justification, a new dependency not GPL-compatible (`cargo deny check`), a
  new network call missing from the wiki's internet list, an incomplete translation.
- **Tests that lie.** A test that freezes current output instead of the intended R-ID behaviour;
  one that still passes after an error fired inside it.

## Confirm, then rank

- **confirmed** = seen (a failing test, a probe's output, a run, or the code read end to end with
  the exact lines). **suspected** = read only, or dependent on state not reproduced. Every finding
  carries one word. Verify the mechanism, not just that a fix is present.
- Rank by who meets it first and how often: a crash or wrong resistance on every ride above a
  wrong number above an owner-only edge case. Mark "by design" findings as such and keep them.

## The report

Save it in the scratchpad as `orqwa_review_<what>_<date>.md`, then paste the whole file in one
fenced block **in the same message as the Write**. Template:

```
# ORQWA: <review of a..b | check of <commits>> (<branch>, <n> commits, <date>)

Review only; nothing in the repo changed. Read: <what>. Ran: <command> (<result>).

## Findings, ranked
1. <One-line claim.> (confirmed | suspected) <file:line>: <what the code does>; <the scenario a
   rider or the owner meets>. Fix: <one line>. Test: <how to prove it>.

## Verified, nothing to do
- <What was checked and holds, with lines.>

## Not reached
- <What was read but not run; what needs a trainer, a GPU or a screenshot.>
```

A check of fixes uses the same template with "Verified" first, new findings marked "low" when
they are, and "Not reached" carried over word for word so nothing is quietly dropped. When the
`developer` session does the fixes, SendMessage it the report as well as pasting it here.
