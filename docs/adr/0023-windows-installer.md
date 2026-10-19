# ADR 0023 — A Windows installer, and updates from within Torqa

- Status: accepted (the installer; the update function follows in a second step)
- Date: 2026-10-19

## Context

Torqa for Windows ships as a zip holding `Torqa.exe` and `torqa_gd.dll`. A rider unpacks it
somewhere, finds it again to start it, and replaces both files by hand for every release. A
rider who wants the newest version for every real ride needs more than that: an install into
Program Files, a Start-menu entry, an uninstaller, and in the end Torqa telling them about a new
version and installing it.

Options considered:

- **An MSI built with the WiX Toolset**: the standard Windows installer format, installed and
  removed like any other app, upgradable in place. Updates are whole installers (about 60 MB).
  WiX is the de-facto tool for building MSIs. It runs only on the Windows CI runner.
- **Velopack**: a `Setup.exe` into the rider's own folder with background delta updates, applied on
  restart. Smaller updates, but no MSI, and a Rust crate plus its own packaging tool.
- **MSIX**: Windows' newer package format, which cannot be installed unless it is signed.
- **The zip only, with an update notice**: no installer at all.

## Decision

- **Installer.** The `windows` CI job builds `Torqa-windows-x86_64.msi` with **WiX v7** from
  `scripts/release/Torqa.wxs`, next to the zip, and every release carries it.
  - **Install.** Per machine into `Program Files\Torqa`, with an advertised Start-menu shortcut,
    the app's icon under Installed apps, and a fixed upgrade code. A newer MSI, or the same
    version again, replaces the installed one; an older one refuses to downgrade.
  - **Version.** The MSI's version is the `.exe`'s (`a.b.c.0`, kept in step with
    `core/Cargo.toml` by `scripts/release/version.py`).
  - **The rider's data stays.** It lives in `%APPDATA%` and `%LOCALAPPDATA%`, never in the
    installation, so installing, upgrading and uninstalling keep rides, riders and settings.
  - **Checked in CI.** The job installs the MSI silently, checks the files and the shortcut,
    and uninstalls it again.
- **WiX's terms.** WiX's source is under the Microsoft Reciprocal License. Since v7 its
  binaries come with the [Open Source Maintenance Fee EULA](https://github.com/wixtoolset/wix/blob/main/OSMFEULA.txt),
  which every build accepts (`wix build -acceptEula wix7`). The fee applies only to users that
  earn at least US$10,000 a year with it; Torqa earns nothing. WiX builds the installer only,
  and none of it ships inside Torqa.
- **Unsigned for now.** Like the `.exe`, the MSI is not code-signed, so Windows' SmartScreen
  warns once per new version ("More info → Run anyway"). Signing (SignPath for open-source
  projects, or Azure Trusted Signing) can follow without changing the installer.
- **Updates (next step).** Torqa will look for a newer release once at start-up, never during a
  ride, behind a setting that turns it off. It will offer to download the new MSI and install it
  over the running version. The servers it contacts will be added to the wiki's internet list
  with that change.

## Consequences

- Windows riders install, update and remove Torqa like any other app, and their data survives
  all three.
- Releases carry one more asset, and the Windows job takes about a minute longer.
- The project depends on WiX's EULA terms staying free for projects without revenue. If they
  change, the installer can be built from WiX's source, which the EULA leaves free.
- macOS and Linux keep their archives for now.
