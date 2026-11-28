# Supported trainers

ORQWA controls Bluetooth smart trainers through **FTMS** or **Tacx FE-C over Bluetooth**.
A trainer that follows either protocol generally works even if it is not listed here. The
table records what has been checked on hardware and what ORQWA implements for it.

| Trainer | Protocol | Status | Notes |
|---|---|---|---|
| Wahoo KICKR CORE 2 | FTMS | Checked on hardware | Power, cadence, speed, slope simulation, ERG, resistance and reconnection |
| Tacx NEO 2 Smart | Tacx FE-C over Bluetooth | Checked on hardware | Power, cadence, speed, slope simulation, ERG, resistance and reconnection (R110, [ADR 0028](adr/0028-tacx-bluetooth.md)) |
| Elite Direto X | FTMS | Checked on hardware | Trainer, power and cadence |
| Cycplus R200 | FTMS | Implemented, hardware check pending | Power, cadence, speed, slope simulation, ERG, resistance and reconnection (R302); the BC2 shifter's native gear notifications are not used yet |
| Fake trainer | Built in | Always available | Development and rides without hardware (R10) |

**Checked on hardware** means the trainer was connected and ridden with ORQWA, not merely
recognized. An unlisted FTMS trainer that offers Indoor Bike Data and a writable Fitness
Machine Control Point can work too; so can another Tacx trainer that speaks FE-C over
Bluetooth. If one does not, please open an issue with a `RUST_LOG=orqwa_devices=debug` log.

## How trainers are found

Discovery treats a device as a trainer when it advertises FTMS (`0x1826`) or Tacx FE-C
(`6e40fec1…`). A device that also advertises the Cycling Power Service stays a trainer rather
than a power meter. FTMS takes precedence when a device offers both protocols.

The Cycplus R200 is also recognized by its advertised name, `R200 <serial>`, because some
platforms leave service UUIDs out of the advertisement.

## What ORQWA sends

**FTMS** uses Indoor Bike Data (`0x2AD2`) for readings and the Fitness Machine Control Point
(`0x2AD9`) for control: Request Control (`0x00`), Set Target Resistance (`0x04`), Set Target
Power (`0x05`), Start or Resume (`0x07`) and Set Indoor Bike Simulation (`0x11`). Supported
Resistance Level Range (`0x2AD6`) is read when the trainer offers it.

**Tacx FE-C** sends ANT-framed pages over the Tacx UART service: simulation alternates wind and
track pages, while ERG and resistance repeat the latest target. No ANT+ radio or dongle is
needed.

Both paths use the same discovery, remembered-device and reconnection behaviour. After a drop,
the last control is restored when the trainer is back.

## Checking a trainer on hardware

Use a native CI-built app or CLI (Docker on macOS has no Bluetooth). Close other trainer apps,
run `orqwa-cli scan`, then connect in the app's Trainer row. With
`RUST_LOG=orqwa_devices=debug`, record the firmware, advertised services, power, cadence and
speed, slope simulation on flat roads, climbs and descents, ERG targets, fixed resistance,
release on pause, virtual gears and reconnection.

A model that passes joins the table above and the wiki's tested devices. Protocol and packet
tests run without a radio with `cargo test -p orqwa-devices`.
