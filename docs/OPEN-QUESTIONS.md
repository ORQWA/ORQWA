# Open questions and findings

Things to review or decide together. Newest first; remove entries once settled. Work that is
decided but not done goes into a GitHub issue instead.

## Tacx NEO 2 Smart hardware check (R110, 2026-10-21)

The FE-C-over-Bluetooth driver is implemented from the documented protocol, without a NEO
at hand. A friend's NEO 2 Smart needs to confirm that it appears in the trainer row, reports
power/cadence/speed, follows climbs and descents, holds ERG targets, changes resistance,
releases on pause and reconnects with its previous target after a drop. Also check virtual
gears on flat roads (wind and rolling-resistance changes). Record its firmware and the
`RUST_LOG=orqwa_devices=debug` log; see ADR 0028. Other Tacx models remain unverified too.

## Battery levels are unchecked on the devices (R97, 2026-10-19)

The Devices tab tells a battery level wherever a device has Bluetooth's Battery Service or,
for a Zwift Click and the Click v2's pods, Zwift's `0x19` message. Only the left Click v2
pod's `0x19` was seen (80 %, #306). Still to see, with `RUST_LOG=orqwa_devices=debug`,
`orqwa-cli ride --fake --hr` for the strap and
`orqwa-cli gear-check --fake --controller <name>` for the shifters (both print
`battery …` as it changes): which of the Polar H9, the Venu 3, the Assioma pedals, the Di2
and the Clicks offer the Battery Service at all, whether reading it makes any of them ask to
pair, and whether the right Click v2 pod tells its level at all.

## Signing the macOS and Windows downloads (2026-10-19)

The macOS app is only signed ad hoc and the Windows `.exe` not at all, so Gatekeeper and
SmartScreen stop riders on the first start of every download (the wiki's Getting started says
how to get past them), and Windows' Smart App Control blocks ORQWA outright. To decide:

- **macOS:** only a Developer ID with notarization avoids it, and that needs the Apple
  Developer Program (US$99 a year). Apple waives it for nonprofits, schools and governments, not
  for individuals or open-source projects. With it, CI would sign through Godot's exporter and
  notarize, with the certificate and API key as secrets.
- **Windows:** SignPath Foundation signs open-source projects for free, in the Foundation's
  name: a maintainer applies, and each signing is approved there. Certum's open-source
  certificate (from €49) names the maintainer. Either satisfies Smart App Control, but
  SmartScreen still warns until the signed releases have built up a reputation.
