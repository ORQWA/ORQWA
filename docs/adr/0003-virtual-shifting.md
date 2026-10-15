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

Implemented in `torqa_physics::VirtualGears`:

- **Gears:** 24, from 0.75 to 5.5 (chainring over cog), each about 9 % above the last. A ride
  starts in the gear nearest to the real one (chainring / cog of the rider's drivetrain,
  50 / 14 by default).
- **Resistance:** in slope simulation the trainer brakes its flywheel, turning at the speed of
  the real gear, as the road would at that speed. In a virtual gear `r` times the real one, the
  same cadence is `r` times the speed, and the pedals feel the road's force at that speed times
  the leverage `r`. With the trainer's force `m·g·(grade + Crr) + Cw·v²`, sending `r·grade`,
  `r·Crr` and `r³·Cw` gives exactly `r·F_road(r·v)` — so gears change how hard the road feels
  at a cadence, while Torqa's virtual speed still comes from the measured power. Crr and Cw
  are capped at what FTMS can carry (0.0255, 2.55 kg/m); the trainer limits the grade.
- **Inputs:** behind `ShiftInput` (`torqa_domain::shifting`): the keyboard (↑ / ↓ in the
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

The unit only says whether a button is down. `torqa_devices::zwift_click` makes the presses
of a Di2 out of that, so the assignments of #139 apply unchanged: a **hold** once the button
has been down for 500 ms (told then, not when let go, so a long hold is one press however
long), a **press** when it comes up before that. There is no double press: waiting for a
second tap would delay every shift, and a shifter must not lag. The + is button 1 and the −
button 2 of the shared button map, so the default (1 shifts up, 2 down) fits a Click as it
fits a Di2, and the Devices tab shows a Click's two rows and two columns.

Still out: the Click v2 (two pods, device types 10 and 11; its left pod stops sending after
about a day without Zwift, its right pod needs further activation writes), the Play (device
types 2 and 3; other button messages) and the Ride. Their protocols are documented by the
same community projects, and would be separate drivers once someone has the hardware to
check them. Zwift has not published any of this and a firmware update may change it; the
driver stays isolated and optional, as this ADR always said. Rouvy and MyWhoosh took the same
route (DC Rainmaker, 2025-02).
