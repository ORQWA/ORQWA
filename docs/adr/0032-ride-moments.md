# ADR 0032 — One channel for the moments of a ride

- Status: accepted
- Date: 2026-10-24
- Requirements: R124, R27, R42, R113

## Context

Riding showed only figures. A climb's top and the finish were short toasts ("Climb done in
14:02 — new record, 0:12 faster!") built in the ride screen from two dedicated events,
`ClimbCompleted` and `RouteCompleted`. Issue #185 asks for more things worth telling the rider:
milestones, the first ride on a course, power peaks, records, habits. A coach (#186) may later
want to speak during the ride as well. Each new kind as its own event and signal would spread
the wording across the ride screen.

## Decision

- The core decides **what happened**, the front end **how it is said**. Detection lives in
  `orqwa-app::moments` and is reported as `AppEvent::Moment(Moment)`, carrying only figures in
  SI units and indexes. `ClimbCompleted` and `RouteCompleted` are gone. A climb is a record
  only when faster than an earlier real time; simulated rides set none.
- Godot has one `moment(info)` signal. `MomentLines` holds every line in one table, two or
  more casual variants per moment and one plain line, so the wording can be refreshed without
  touching detection. Lines are written for people: full sentences, no slang that ages.
- The rider's **voice** (Casual, Plain, Off) is part of the profile, as the map style is,
  because riders sharing an installation may want different ones.
- The ride screen shows moments on a `MomentCard`, one at a time; the overlay shows the line
  in its bar. The finish is not a card: the quick summary (R113) opens over the finished ride
  at once and tells it there.
- Milestones follow the rider's units: 25 km and 1 km to go, or 25 miles and 1 mile.
- Power records are found live against the bests of the rider's history, loaded at the
  start, and told once the effort tops out rather than every second it keeps rising. A rider
  without records gets none mid-ride: every effort would be one.

## Consequences

- A future source of messages, such as a coach, reuses the card and the voice.
- With the voice off, the climb toasts riders had before are gone too; the climb panel still
  shows the time and the best.
