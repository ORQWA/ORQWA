# ADR 0017 — Classified failures, translated at presentation

- Status: accepted
- Date: 2026-10-15

## Context

Core errors were English strings displayed verbatim, although the rider selected German
(#261, R74). Translating interpolated strings would require matching diagnostics from Rust
libraries, servers and operating systems, and could accidentally translate filenames.

## Decision

Carry `Failure { kind, args, details }` through asynchronous jobs and application events.
Classify known errors at their operation boundary. `Kind::message()` supplies a gettext key;
Godot translates it using the rider's language before substituting untouched arguments.
Unknown causes use a translated fallback rather than appearing as the primary explanation.
Retain the original diagnostic in `details`, `Display` and logs for troubleshooting/CLI use.

Keep the existing Godot error signals as localized summaries. Add `failure_reported` and
`last_failure()` for structured presentation. A collapsed **Technical details** control keeps
the latest 20 failures available for the session, including after ride notifications fade.
Connection dialogs also expose details inline. Changing language retranslates stored keys.

## Consequences

- UI code never parses formatted errors or decides business failure classifications.
- Third-party diagnostics may remain English inside the separate details area.
- New failure operations need a classification and translated template; placeholder values
  must never be reinterpreted as formatting instructions.
- Gettext contexts distinguish the ride action from the ride settings tab. The individually
  reviewed German wording lives directly in the catalogue, with completeness checks.
