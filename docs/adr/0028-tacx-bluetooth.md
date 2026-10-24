# ADR 0028 — Tacx FE-C over Bluetooth

- Status: accepted; confirmed on a NEO 2 Smart (2026-10-24)
- Date: 2026-10-21

## Context

A rider's friend has a Tacx NEO 2 Smart which does not offer the FTMS control protocol
ORQWA used. Tacx documents an FE-C tunnel over Bluetooth. This is separate from native
ANT+ support, which was dropped; it uses the existing Bluetooth adapter and no dongle.

## Decision

- Add `orqwa_devices::tacx`, using Tacx's
  [How-to FE-C over BLE v1.0.0](https://github.com/jedla22/BleTrainerControl/blob/master/How-to%20FE-C%20over%20BLE%20v1_0_0.pdf)
  and the FE-C field layouts cross-checked against GoldenCheetah and FortiusANT.
  The codec is original code; no new dependencies or runtime network calls.
- Discover the advertised `6e40fec1-b5a3-f393-e0a9-e50e24dcca9e` service as a trainer,
  before Cycling Power. Subscribe to `…fec2…` and write complete ANT messages to `…fec3…`.
  Prefer FTMS where both services exist, regardless of advertised device name.
- Introduce the `TrainerDriver` control-encoding trait for the two protocols. The shared
  Bluetooth driver continues to own the device handle, subscriptions and reconnection.
  Existing callers see the same controllable trainer and telemetry events.
- Validate ANT sync, length, message type and XOR checksum. Read speed from page 16 and
  power/cadence from page 25, masking power's status bits; an unavailable value is unknown,
  not zero. The trainer sends the two pages in turn, so each passes on the latest of all
  three readings, as one FTMS notification does: the Devices tab shows only the last
  reading (R80). Unknown pages contribute no readings; rejected commands on page 71 are
  logged.
- Encode resistance (page 48, half-percent), ERG (49, quarter-watts) and simulation
  (50 for wind, 51 for grade/rolling resistance). Clamp to valid FE-C ranges, excluding
  unavailable-value sentinels. Grade uses a +200 percent offset; wind uses km/h +127.
- Repeat the current control at Tacx's recommended 4 Hz, alternating simulation pages;
  replace queued old targets on a mode change. On reconnect restore the desired control,
  otherwise start at zero resistance. No FTMS request/start handshake on Tacx.
- Reuse the existing pairing screen and saved device identifiers. No screen layout or
  screenshot changes are needed.

## Consequences

Other Tacx units advertising the same service can use this adapter, but only the NEO 2
Smart has been tried. Native ANT+, Tacx road feel and calibration are outside this change.
FE-C simulation uses the trainer's configured/default rider mass, as ORQWA's current
simulation parameters do not carry mass. Each further model needs its own hardware check:
uphill, flat and downhill feel, wind/rolling resistance with virtual gears, ERG, release on
pause, and restoration after a dropped connection.
