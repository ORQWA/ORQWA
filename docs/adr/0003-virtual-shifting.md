# ADR 0003 — Virtual shifting

- Status: accepted
- Date: 2026-10-02

## Context

The KICKR Core 2 can be used with a normal cassette or with a single Zwift Cog (R9). With a cassette,
real shifting works without app involvement: in SIM mode the app sends the gradient and the trainer
sets resistance based on flywheel speed. With a Zwift Cog, gears must be simulated.

Zwift's own virtual shifting runs over the proprietary "Zwift protocol" that requires trainer
firmware support and has no published specification. Zwift initially promised an open protocol
for the Click, then walked this back. Rouvy and the open-source BikeControl project use
reverse-engineered implementations; BikeControl also defines the open OpenBikeControl protocol.

## Decision

- Implement **app-side virtual gears** over standard FTMS: the selected virtual gear ratio adjusts
  the resistance/grade sent to the trainer. Works with any FTMS trainer, not just Zwift-protocol ones.
- Shift input is a separate `ShiftInput` trait with implementations for keyboard/gamepad,
  OpenBikeControl, and Zwift Click via reverse-engineered BLE (isolated, optional, may break with
  firmware updates).
- Virtual shifting is optional and post-MVP; with a cassette it stays off.

## Consequences

- No dependency on Zwift's proprietary protocol for resistance control.
- Click support carries maintenance risk; check BikeControl's license before reusing any code.

## Amendment 2026-10-06: how the gears feel

Implemented in `orqwa_physics::VirtualGears`:

- **Gears:** 24, from 0.75 to 5.5 (chainring over cog), each about 9 % above the last. A ride
  starts in the gear nearest to the real one (chainring / cog of the rider's drivetrain,
  50 / 14 by default).
- **Resistance:** in slope simulation the trainer brakes its flywheel, turning at the speed of
  the real gear, as the road would at that speed. In a virtual gear `r` times the real one, the
  same cadence is `r` times the speed, and the pedals feel the road's force at that speed times
  the leverage `r`. With the trainer's force `m·g·(grade + Crr) + Cw·v²`, sending `r·grade`,
  `r·Crr` and `r³·Cw` gives exactly `r·F_road(r·v)` — so gears change how hard the road feels
  at a cadence, while ORQWA's virtual speed still comes from the measured power. Crr and Cw
  are capped at what FTMS can carry (0.0255, 2.55 kg/m); the trainer limits the grade.
- **Inputs:** behind `ShiftInput` (`orqwa_domain::shifting`): the keyboard (↑ / ↓ in the
  app, `u` / `d` in the CLI) and **Shimano Di2** (R59): the 12-speed Di2 or the EW-WU111
  wireless unit indicates presses of the buttons assigned to D-Fly channels (service
  `0x18EF`, characteristic `0x2AC2` in Shimano's own UUID base; `[counter, ch1…ch4]`, flags
  for short, long and double presses). Each channel's byte keeps its last press until the
  next one (only a long press is released), so a press is a channel whose byte changed, not
  every flagged channel of a new indication — on a real RD-R8150 that miscounted presses on
  the other channel. Each channel's press, hold and double press do what the rider gives
  them (#139): shift one or two gears, or a ride control the front end carries out as its key
  (next camera, overlay, music, pause) or through the core (R66: trainer difficulty ±10 %,
  workout intensity ±5 %, skip a structured workout's step). `ShiftInput::controls` passes the controls on beside the
  shifts. Until assigned, the channels chosen to shift up and down do so, one gear a press or
  hold and two a double press.
  The Zwift Click is not supported: its protocol is encrypted, the v2 must be unlocked in
  Zwift each day, and the bridge app that offers it (BikeControl) needs a paid subscription.
- In ERG (workouts) the trainer holds the power in any gear.

## Amendment 2026-10-15: the Zwift Click (#228, R75)

R59 left the Zwift Click out as encrypted and unlocked daily in Zwift. That describes the
two-pod Click v2 and the early firmware of the Play and the original Click. Checked on an
original Click (the single unit with + and −) on 2026-10-15, from a Windows PC's own
Bluetooth: the unit speaks the plain protocol the community documented for the Zwift Ride
([Makinolo](https://www.makinolo.com/blog/2024/07/26/zwift-ride-protocol/)) and for the Click
([click_bridge](https://github.com/imcczy/click_bridge), MIT). No key exchange, no pairing, no
unlock, no keepalive:

- Service `00000001-19ca-4651-86e5-fa29dcdd09d1`; notifications on `…0002`, writes without
  response on `…0003`, read and indications on `…0004`. Advertised as "Zwift Click", with
  Zwift's company id `0x094A` and device type 9 where the stack passes the advertising data on.
- Handshake: write ASCII `RideOn` to `…0003`; the unit indicates `RideOn` on `…0004` (and
  answers reads of it with the same once the session is up, empty before). A unit on the old
  encrypted firmware would answer with `RideOn` plus two bytes and its public key instead; the
  driver logs that and shifts nothing, rather than carrying a second protocol.
- Buttons on `…0002`: message `0x37`, then protocol-buffer varints, field 1 the + and field 2
  the − button, `0` down and `1` up: `37 08 00 10 01` is + down, `37 08 01 10 00` − down,
  `37 08 01 10 01` both up. The unit repeats the state about ten times a second while a
  button is down and for a few seconds after, so the driver acts on changes, never on frames.
  Every few seconds `19 10 5D`: a status whose field is the battery (93 %); the standard
  Battery Service is there too.

The unit only says whether a button is down. `orqwa_devices::zwift_click` makes the presses
of a Di2 out of that, so the assignments of #139 apply unchanged: a **hold** once the button
has been down for 500 ms (told then, not when let go, so a long hold is one press however
long), a **press** when it comes up before that. There is no double press: waiting for a
second tap would delay every shift, and a shifter must not lag. The + is button 1 and the −
button 2 of the shared button map, so the default (1 shifts up, 2 down) fits a Click as it
fits a Di2, and the button dialog shows a Click's two rows and two columns.

Still out: the Click v2 (two pods, device types 10 and 11; its left pod stops sending after
about a day without Zwift, its right pod needs further activation writes), the Play (device
types 2 and 3; other button messages) and the Ride. Their protocols are documented by the
same community projects, and would be separate drivers once someone has the hardware to
check them. Zwift has not published any of this and a firmware update may change it; the
driver stays isolated and optional, as this ADR always said. Rouvy and MyWhoosh took the same
route (DC Rainmaker, 2025-02).

## Amendment 2026-10-16: the Zwift Click v2 (R77)

The Click v2 followed after all, as a driver of its own (`orqwa_devices::zwift_click_v2`)
written from the community's drivers, with **no pod at hand**: it is marked unchecked until a
Click v2 owner confirms it (OPEN-QUESTIONS). What those drivers agree on:

- **Two devices.** Each pod advertises on its own as "Zwift Click", Zwift's company id
  `0x094A` and device type **10** for the right pod (+, Y, Z, A, B) and **11** for the left
  (−, ↑, ↓, ←, →), with the original Click's characteristics under the Ride's service
  `0000fc82-0000-1000-8000-00805f9b34fb`, not the Click's: a maintainer's pods answered the
  first attempt with `device lacks service 00000001-19ca…` (#271, 2026-10-16), and BikeControl
  looks for the Ride's service first and the Click's second, so the driver takes either. Zwift's own app
  takes one connection, the left pod relaying the right and the trainer; third-party apps
  connect each pod. ORQWA joins the two found in a scan into one device, "Zwift Click v2", the
  right pod first and the left as its partner, driven over one handle: one row in the Devices
  tab, one remembered device, both pods dropped and reconnected together. A pod alone is the
  device by itself. Without manufacturer data a pod cannot be told from an original Click and
  is taken for one; that pod would answer the handshake and then say nothing ORQWA decodes.
- **The Ride's plain protocol**, not the Play's encryption: write `RideOn` to `…0003`; the
  pod indicates `RideOn 02 03 …` on `…0004`. Buttons on `…0002` as message `0x23`, field 1 a
  varint of a 32-bit map, a bit **clear while its button is down** (`23 08 FF FF FF FF 0F` is
  all up): ← `0x1`, ↑ `0x2`, → `0x4`, ↓ `0x8`, A `0x10`, B `0x20`, Y `0x40`, Z `0x80`, −
  `0x100`, + `0x1000`, as BikeControl's code reads them (its generated protobuf enum has Z and
  the shifts one bit further; the code's table is the one that ships). Idle messages `0x15`
  and `0x19` say nothing of the buttons. Each pod reports its own buttons, so the driver only
  reads a pod's own bits from its frames.
- **The left pod is locked** to Zwift
  ([BikeControl's account](https://bikecontrol.app/blog/zwift-click-v2-with-other-trainer-apps/)):
  unless it was in a Zwift session within about 24 hours, it answers the handshake with a
  vendor challenge `ff 03 …` instead of `RideOn` and tells of no buttons. Unlocking is logging
  in to Zwift (no subscription needed), connecting the Click, closing Zwift; BikeControl can
  also relay Zwift's unlock, which ORQWA does not. After the handshake ORQWA writes `ff 04 00`
  to a left pod, as QZ does unconditionally: it keeps an unlocked pod unlocked across
  connections and does nothing to a locked one. A challenge is logged as the pod being
  locked. The right pod is never locked, so + and a lettered button given "shift down" shift
  without Zwift at all, as BikeControl's "right side only" mode does.
- **Pods fall asleep** after roughly a minute without traffic and drop the link
  (QZ, [#4743](https://github.com/cagnulein/qdomyos-zwift/pull/4743): HCI supervision
  timeout after ~56 s). QZ writes `RideOn` again every 3 s to both pods; ORQWA does the same.
  MaximumTrainer tried 3, 6 and 20 s and none, suspecting the re-sends of causing drops, so
  this is the point to check first on hardware.

The presses come from the same state machine as the original Click's (a hold at 500 ms, a
press when let go before, no double press). The buttons are numbered + 1 and − 2, so the
default map fits, then Y, Z, A, B, ↑, ↓, ←, → as 3–10: the button map grew from four to ten
buttons for it (settings files from before keep their four channels). Sources: BikeControl
(`lib/bluetooth/devices/zwift/`, non-commercial license: read for the protocol, nothing
copied), QZ's `zwiftclickremote.cpp` (GPL-3.0), MaximumTrainer Redux PR #307 and Makinolo's
[Ride protocol](https://www.makinolo.com/blog/2024/07/26/zwift-ride-protocol/). The Play and
the Ride stay out.

## Amendment 2026-10-16: the buttons in a dialog (#271)

The button assignments left the Devices tab for a dialog behind a pen beside the shifter's
row. A Click v2's ten rows (twenty cells) made the tab taller than a screen, and the tab does
not scroll; a Di2's twelve cells were already most of it. The dialog lays the rows out for the
shifter chosen, clamped to the screen with a scroll bar inside, and the pen is disabled while
no shifter is chosen. The other rows keep the pen's column empty, so their Search buttons and
states line up.


## Amendment 2026-10-19: the Click v2 checked on a pair (R96)

A maintainer's pair answered the driver as the community's drivers led us to expect
(`orqwa-cli gear-check --controller "Zwift Click v2"` with `RUST_LOG=orqwa_devices=debug`,
two minutes, each button pressed once):

- **Handshake.** Both pods answer `RideOn` on `…0004`, at once and to every keepalive; the
  answers after a pod's first go to the trace log only, as they come every 3 s.
- **Buttons.** Each of the ten buttons cleared the bit BikeControl's code reads for it, so the
  table from its protobuf enum is the wrong one. + and − shifted once per press.
- **The left pod relays the right.** Its `0x23` frames carry the right pod's buttons too,
  within about 30 ms of the right pod's own frames; the right pod's frames carry only its own.
  Reading each pod's own bits from its own frames is what keeps a press on the right pod from
  counting twice, now tested with the pair's frames.
- **Keepalive.** With `RideOn` every 3 s the link held for the whole run, 77 s of it without a
  press, and nothing complained. Whether the pods would fall asleep without it was not tried.
- **No lock seen.** The left pod sent its buttons although, as far as its owner remembers, it
  was never connected to Zwift. It did send one 85-byte `ff 03 …` five seconds after
  connecting, on `…0002` rather than as the handshake's answer on `…0004`, and went on
  sending its buttons. Whether this firmware locks at all, or only after a first Zwift
  session, stays unknown; the `ff 04 00` acknowledgement stays, as it harms nothing.
- **Other messages on `…0002`**, from the left pod only, none of them buttons: `0x19` every
  5 s, the battery level in its field 2 (80 % here, as
  [jat255's Click client](https://github.com/jat255/Zwift_click_handling) reads it), and after
  each keepalive a `0x2A` status and an `ff 05 …` holding the pod's serial number. So `ff 05`
  is no stop there; the driver reads it as one only on `…0004`, where it never came.

## Amendment 2026-10-19: battery levels (R97)

The Devices tab tells how full a device's batteries are. A Zwift Click's and a Click v2 pod's
`0x19` message on `…0002` carries the level in its field 2, and the drivers now read it beside
the buttons. Every device's level may also come from Bluetooth's Battery Service (`0x180F`,
level `0x2A19`), read while connecting and followed where the device notifies; a Click v2 pod
offering it tells its own side's. The read is bounded (3 s), so a battery never holds up a
connection. A driver tells a level only when it changes, since a pod repeats its own every few
seconds. Only the left pod's `0x19` has been seen (80 %); the right pod sent none in two
minutes (OPEN-QUESTIONS).

## Amendment 2026-10-19: virtual gears by default (R98)

A new rider starts on a single cog (50 × 14) instead of a cassette. With a cassette as the
default, a Zwift Click connected for a first ride did nothing: its presses only shift virtual
gears, and nothing said why. Riders with a cassette choose it once in the profile; profiles
saved before keep the drivetrain they had. The drivetrain can now also be changed during a
ride, with the rest of the profile on the ride settings' Profile tab: the profile keeps it, and
the ride takes the new gears at once. Switched from a cassette to virtual gears, the ride starts
from the gear nearest the real one. Other teeth keep the gear ridden: the 24 virtual ratios do
not depend on the real gear, only how much the trainer scales the road for them does.

Checked on a KICKR CORE the same day (`orqwa-cli gear-check`, Zwift Click v1 shifting): the
trainer follows the scaled Crr and Cw as well as the grade. On the flat, gear 24 held 18.5 km/h
at about 100 W and gear 5 24 km/h at about 40 W, where the road alone would take about 44 and
79 W. The easiest gears cannot go below what the trainer brakes at the least.
