# Changelog

All notable changes are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-14

First public release.

### Added

- Hotkey-toggled calculator popup with live results, powered by fend: arithmetic, units,
  percentages, number bases, variables and `ans`.
- Enter copies the result; history with ↑/↓ and click-to-copy, saved across sessions.
- Currency conversion with daily rates from exchange-api, cached for offline use.
- GNOME install script that binds the hotkey (default Alt+Space).
- Downloads: a .deb for double-click install on Ubuntu/Debian, and a tar.gz for any distro;
  both include all third-party licence texts.
- App-grid entry and icon; on first launch from GNOME, a banner offers to set up Alt+Space.
- Fallback font so arrows and math symbols render on Debian/Ubuntu, Fedora and Arch.

[0.1.0]: https://github.com/delirious6423/spotcalc/releases/tag/v0.1.0
