# spotcalc

A Spotlight-style calculator popup for Linux. Press a hotkey, type a calculation, read the answer
as you type, press Enter to copy it. Press the hotkey again (or Esc) and it's gone.

```
15% of 80          12
3 ft to cm         91.44 cm
0xff to binary     11111111
100 USD to TWD     3213.18 TWD   (with that day's rate)
x = 4              4
x^2 + ans          20            (ans = previous result)
```

## Features

- **Instant answers**: the result appears while you type; Enter copies it to the clipboard.
- **Units and conversions**: lengths, weights, temperatures, speeds, data sizes, time, and more.
- **Currencies**: 160+ currencies with daily exchange rates, cached so conversions work offline.
- **Programmer-friendly**: hex, octal and binary input and output.
- **Variables and `ans`**: `rate = 0.05`, then `1200 * rate`; `ans` is the previous result.
- **History**: ↑/↓ to recall earlier calculations; click one to copy its result.
- **Light on resources**: opens in about 0.1 s and uses no CPU while open and idle.

Calculations are done by [fend](https://github.com/printfn/fend), an arbitrary-precision,
unit-aware calculator; see the [fend manual](https://printfn.github.io/fend/documentation/) for
everything it understands.

## Keys

| Key | Action |
| --- | --- |
| Enter | Calculate, copy the result, save it to history |
| ↑ / ↓ | Browse history |
| Esc, the hotkey again, or clicking elsewhere | Close |

## Requirements

- Linux on x86_64 with glibc 2.35 or newer (Ubuntu 22.04+, Debian 12+, Fedora 36+, or any
  current rolling distro), a Wayland or X11 desktop, and OpenGL (any GPU driver from the last
  decade).
- Optional: the DejaVu Sans font (installed by default on most distros) for symbols such as
  ↑ √ ×. Without it they show as boxes.

## Install

### Download (no Rust needed)

Get `spotcalc-<version>-x86_64-linux.tar.gz` from the
[releases page](https://github.com/delirious6423/spotcalc/releases), then:

```bash
tar -xzf spotcalc-*-x86_64-linux.tar.gz
cd spotcalc-*-x86_64-linux
./install.sh '<Alt>space'
```

This copies `spotcalc` to `~/.local/bin` and, on GNOME, binds the hotkey (see below). On other
desktops it just installs the binary; bind a key as described under
[Other desktops](#other-desktops). Each `.tar.gz` has a `.sha256` file next to it for checking
the download (`sha256sum -c spotcalc-*.tar.gz.sha256`).

### Build from source

Needs [Rust](https://www.rust-lang.org/tools/install) 1.92 or newer and a C compiler
(`build-essential` on Debian/Ubuntu, `gcc` elsewhere).

```bash
git clone https://github.com/delirious6423/spotcalc
cd spotcalc
scripts/install.sh '<Alt>space'    # or just: cargo install --locked --path .
```

The binary goes to `~/.cargo/bin`. Running `spotcalc` opens the window; running it again while
it is open closes it, so a hotkey that runs it toggles the window.

### GNOME

`install.sh` adds a custom keyboard shortcut (default Alt+Space), shown afterwards in
Settings → Keyboard → Custom Shortcuts. GNOME uses Alt+Space for the window menu, so the script
takes that key off the window menu (you can still right-click a title bar). On machines whose
only GPU is NVIDIA, it also makes the shortcut skip loading Mesa's graphics driver, which saves
about 50 MB of memory.

To remove the shortcut and the binary (history and cached rates stay in the folders listed under
[Files](#files)), run `./install.sh --uninstall` from the download, or
`scripts/install.sh --uninstall` from the source tree.

### Other desktops

Bind a key to run `~/.cargo/bin/spotcalc`:

- **KDE Plasma**: System Settings → Keyboard → Shortcuts → Add New → Command or Script.
- **Sway / i3**: `bindsym Mod1+space exec ~/.cargo/bin/spotcalc` and, so it floats instead of
  tiling, `for_window [app_id="spotcalc"] floating enable`.
- **Other tiling compositors**: run the same command from a key binding and add a window rule
  making app id / class `spotcalc` floating.

## Exchange rates and privacy

Currency rates come from fawazahmed0's
[exchange-api](https://github.com/fawazahmed0/exchange-api) (free, no API key, updated daily).
spotcalc downloads them in the background at most every 12 hours, from jsDelivr with the
project's Cloudflare mirror as fallback, and caches them so conversions work offline with the
last rates. Those servers see your IP address and the time of the request; nothing you type
ever leaves your computer. If you never use currencies, the only network traffic is that one
small download.

## Files

| Path | Contents |
| --- | --- |
| `~/.local/share/spotcalc/history.tsv` | Last 200 calculations (`$XDG_DATA_HOME` if set) |
| `~/.cache/spotcalc/rates.json` | Cached exchange rates (`$XDG_CACHE_HOME` if set) |

## Troubleshooting

- **The result is gone from the clipboard after the window closes.** On Wayland, clipboard
  contents belong to the app that copied them. GNOME keeps them after the app exits; on other
  desktops, run a clipboard manager.
- **The window opens but doesn't take keyboard focus.** Some desktops prevent apps started from
  a shortcut from taking focus. Click the window once; please open an issue naming your desktop.
- **Symbols show as boxes.** Install DejaVu Sans (`fonts-dejavu-core` on Debian/Ubuntu,
  `dejavu-sans-fonts` on Fedora, `ttf-dejavu` on Arch).
- **Logs**: run `RUST_LOG=spotcalc=debug spotcalc` from a terminal.

## How it works

Wayland doesn't let apps grab global hotkeys, so the desktop's shortcut simply runs `spotcalc`.
The running instance listens on a Unix socket in `$XDG_RUNTIME_DIR`; a second launch connects to
it, which tells the open window to close, so one key toggles the window.

| File | Role |
| --- | --- |
| `src/calc.rs` | fend wrapper: live preview and time limits on slow calculations |
| `src/rates.rs` | Exchange-rate download, cache and lookup |
| `src/history.rs` | History file |
| `src/instance.rs` | Single-instance toggle socket |
| `src/app.rs` | The egui window |

## Contributing

Bug reports and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Copyright 2026 spotcalc contributors. Licensed under the [Apache License, Version 2.0](LICENSE).

Release downloads include `THIRD_PARTY_LICENSES.txt` (generated by
[cargo-about](https://github.com/EmbarkStudios/cargo-about)) with the licence of every library
compiled into the binary, and `licenses/fonts/` for the built-in fonts.

spotcalc builds on [fend](https://github.com/printfn/fend) (MIT),
[egui](https://github.com/emilk/egui) (MIT or Apache-2.0), and exchange rates from
[exchange-api](https://github.com/fawazahmed0/exchange-api) (CC0-1.0). The binary embeds egui's
default fonts: Ubuntu Light ([Ubuntu Font Licence](https://ubuntu.com/legal/font-licence)), Hack
(MIT and Bitstream Vera licence), Noto Emoji ([SIL OFL 1.1](https://openfontlicense.org)), and
an MIT-licensed icon font.
