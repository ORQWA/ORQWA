# Open questions and findings

Things to review or decide together. Newest first; remove entries once settled. Work that is
decided but not done goes into a GitHub issue instead.

## Signing the macOS and Windows downloads (2026-10-19)

The macOS app is only signed ad hoc and the Windows `.exe` not at all, so Gatekeeper and
SmartScreen stop riders on the first start of every download (the wiki's Getting started says
how to get past them), and Windows' Smart App Control blocks Torqa outright. To decide:

- **macOS:** only a Developer ID with notarization avoids it, and that needs the Apple
  Developer Program (US$99 a year). Apple waives it for nonprofits, schools and governments, not
  for individuals or open-source projects. With it, CI would sign through Godot's exporter and
  notarize, with the certificate and API key as secrets.
- **Windows:** SignPath Foundation signs open-source projects for free, in the Foundation's
  name: a maintainer applies, and each signing is approved there. Certum's open-source
  certificate (from €49) names the maintainer. Either satisfies Smart App Control, but
  SmartScreen still warns until the signed releases have built up a reputation.
