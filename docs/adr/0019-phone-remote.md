# ADR 0019 — A remote in the browser of a phone or tablet

- Status: accepted
- Date: 2026-10-18

## Context

Without a Shimano Di2 or a Zwift Click, a rider shifts the virtual gears from the keyboard
(#228), which is out of reach on the bike. R25 left a phone companion app for later.
OpenBikeControl through BikeControl needs a paid subscription and Zwift's own controllers are
locked (R59), so neither helps. Most riders have a phone or a tablet at hand anyway.

A native companion app means a second code base (Swift and Kotlin, or a cross-platform
toolkit), an Apple developer account, review in two stores, and keeping its protocol in step
with every Torqa version. Torqa has no server of its own (R3) to pair the two through.

## Decision

**A page Torqa serves on the local network** (`torqa-remote`, R86), opened in the browser of
a phone or tablet ("the phone" below stands for either). The Torqa a rider runs serves the page
too, so the two never disagree, and any phone, tablet or computer works.

- **A shift input with assignable buttons.** `torqa_remote::Remote` implements `ShiftInput`.
  The page has ten buttons, two big ones first. The phone remote is a `ControllerModel` like
  the shifters (`PhoneRemote`), so its buttons are the rider's own (R84). The ride's state
  that the page hears carries each button's action, by the name a shifter's button gives it
  (`ButtonAction::name`, moved from storage to the domain). A press names its button, and the
  server maps it through the actions it last showed, so the phone presses what it displays.
  Presses only: a hold would make every press wait for the finger to lift. The
  assignments every shifter shared before R84 are not carried to the phone (`SHIFTERS`). The
  app polls the remote beside the keyboard and the Bluetooth shifter. It does not keep it in
  their list, whose second place belongs to the shifter.
- **Plain HTTP/1.1, hand-written** on Tokio, as the OAuth login's loopback answer already is
  (`torqa-upload`). Presses are `POST`s and the page hears the ride (gear, paused, workout) as
  **Server-Sent Events**. A WebSocket would need SHA-1 and base64 for its handshake, so a
  crate, and the page only listens. Only the method and the path of a request are read; heads
  over 8 kB, requests slower than 3 s and more than 16 connections are turned away. The page
  is one embedded HTML file with inline script and style. It loads nothing from elsewhere and
  sends no referrer (a strict Content-Security-Policy).
- **Off until turned on**, per computer (R78, `computer.toml`), on all IPv4 addresses at a
  **fixed port** (8571 until the rider chooses another), so a bookmarked page keeps working.
  The address shown is the one the computer's way out goes through, found with an unsent UDP
  "connect".
- **A secret token** starts every path: 64 random bits from the standard library's
  randomly-keyed hasher, compared in constant time. A wrong or missing token gets the same
  404 as a wrong path. That keeps out others on the network and websites in the rider's own
  browser (cross-site requests, DNS rebinding), so the `Host` header is not checked: riders may
  open the computer by its name. A **new address** restarts the remote with a new token. The
  old phones' streams end, and their page says the address no longer works.
- **Pairing by QR code** with `fast_qr` (MIT, no dependencies with its default features off,
  maintained in 2026). Rust builds the modules and Godot only draws them.
- **A phone counts as connected** while its event stream is open and it was heard from within
  25 s: the page says it is there every 10 s. A locked phone stops talking long before its
  connection notices.
- **Texts come from the front end**, translated with `tr()` like everything else (R24), and
  are embedded in the page with every `<` escaped. The header's logo is the app's own mark
  and wordmark (`app/assets/brand/`), inlined as SVG.

## Consequences

- No app, no store, no account: riders scan a code.
- **Plain HTTP:** anyone who can watch the network's traffic can read the token and press.
  That is acceptable for shifting gears, and a new address locks them out again. TLS would
  need a certificate that phones trust for a local address, which is not to be had without a
  server or a warning in the browser.
- Not a secure context, so the browser offers no Wake Lock: the page asks the rider to turn
  the phone's auto-lock off. There is no installable web app (service worker) either.
- macOS and Windows ask once whether Torqa may accept connections. Guest networks that keep
  devices apart, and phones on mobile data, cannot reach the page.
- A touch screen has no feel of a button, and a web page cannot use the phone's volume keys.
  A Bluetooth shifter stays the better choice where there is one.
- `connected()` can lag a phone that vanished without a word by up to 25 s.
