# ADR 0022 — The interface at the screen's scale, with layouts that reflow

- Status: accepted
- Date: 2026-10-19

## Context

Until now, Godot stretched the whole interface to the window (`canvas_items` with `expand`,
designed at 1600 × 900). Making the window smaller made every text smaller, soon too small
to read, and making it larger only enlarged the text. On a Retina Mac the window also opened
at 800 × 450 points, since Godot opens it at the project's size in pixels. The layouts were
never seen narrower than 1600 units, so many had fixed widths that only fit there (R53).

## Decision

**One interface unit is as large as the system's own interface on the window's screen**
(R94). The stretch mode is off, and `UiScale` sets the window's `content_scale_factor`:

- **The system's scale**: `screen_get_scale` on macOS and Wayland, the screen's DPI over 96 on
  Windows (its display scaling). X11 has no scale of its own, so `UiScale` guesses one from
  the screen's size and DPI, as Godot's editor does.
- **Again on another screen**: the factor is checked whenever the window moves or its DPI
  changes. Windows sends no DPI event, but moving the window is enough.
- **Never too large for the screen**: the scale is lowered only where the smallest window,
  **1024 × 640 units**, would not fit on the screen at it. A 1920 × 1080 laptop at Windows'
  150 % has about 1280 × 690 units, which is why the smallest window is not 1280 × 720. The
  window's minimum size is that smallest window.
- **A new window** opens at the project's 1600 × 900 in units rather than pixels, and on 90 %
  of a screen too small for that.
- **The overlay keeps scaling** with its window (#124): it switches to `canvas_items` and a
  factor of 1 while it is on, and puts the interface's mode back after. Its sizes are relative
  to the interface's scale rather than the screen's points.

**The layouts reflow** instead of scaling:

- `ReflowGrid`, a `GridContainer` that puts as many of its children side by side as fit, up
  to `max_columns`, and the rest below. A container only knows the room it gets, not the room
  there is, and a row too wide for its parent makes the parent as wide as the row. So an
  exact fit counts as too narrow, and the grid settles on the most columns that fit. An
  `expand_room` keeps a map or chart from being squeezed to a strip beside its figures.
  Unlike a `FlowContainer`, its columns line up and a child that expands also fills the
  height.
- Toolbars wrap in groups that stay together (an `HFlowContainer`). Columns side by side
  share the width by stretch ratio rather than a fixed width. Fields have a least width and
  cut their text (`UiTheme.narrowable`) or wrap it. Pages and dialog tabs taller than a small
  window scroll (`UiTheme.scrolling`).
- Dialogs open through `UiScale.popup`, as large as asked where the window has room, else as
  large as fits under their title. Text that wraps measures its height at the dialog's width
  before it opens, and the dialog grows to that height and stays there. So the dialog's size
  is set again a frame later.

## Consequences

- Text keeps its size: a smaller window shows less, a larger one or full screen shows more.
- Every new page and dialog has to work at 1024 × 640, in English and German.
  `window_smoke.gd` checks every tab, a course, the summary, the ride and the large dialogs
  at that size. `SCREENSHOT_SIZE=1024x640 scripts/screenshots.sh` shows them.
- Headless, Godot's window is 100 × 100 units, where before it was always laid out at
  1600 × 900. Tests that click or measure set the window's size themselves.
- On a TV, the text is as large as the system makes it there, and no longer grows with the
  screen. Riders who want it larger raise the system's scale. Under X11 they cannot, until
  Torqa has a size setting of its own.
