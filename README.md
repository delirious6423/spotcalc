# spotcalc

A Spotlight-style calculator popup for GNOME (Wayland or X11). Press a hotkey, type, read the
live result, press Enter to copy it.

Calculation is done by [fend](https://github.com/printfn/fend): arithmetic, units (`3 ft to cm`,
`60 mph to km/h`), currencies (`100 USD to TWD`, `50 eur in jpy`), percentages (`15% of 80`), bases (`0xff to binary`), dates, variables
(`x = 4`, then `x^2`) and `ans` for the previous result.

## Keys

| Key | Action |
| --- | --- |
| Enter | Calculate, copy the result, save to history |
| ↑ / ↓ | Browse history |
| Esc, hotkey again, or click away | Close |

Clicking a history row copies its result. History: `~/.local/share/spotcalc/history.tsv`.

## Exchange rates

Currency rates come from [exchange-api](https://github.com/fawazahmed0/exchange-api) by
fawazahmed0 (free, no key, updated daily), fetched from jsDelivr with the Cloudflare mirror
`currency-api.pages.dev` as fallback. spotcalc refreshes them in the background at most every
12 hours and caches them in `~/.cache/spotcalc/rates.json`, so conversions work offline with the
last rates. The rate servers see your IP address and request time, nothing else.

## Install

```bash
scripts/install.sh '<Alt>space'
```

Builds into `~/.cargo/bin/spotcalc` and adds a GNOME custom shortcut (visible in
Settings → Keyboard → Custom Shortcuts). `<Alt>space` is taken off GNOME's window menu. Undo with `scripts/install.sh --uninstall`.

## How it works

Wayland doesn't let apps grab global hotkeys, so the GNOME shortcut simply runs `spotcalc`. The
running instance listens on `$XDG_RUNTIME_DIR/spotcalc.sock`; a second launch connects to it,
which closes the open window, so the hotkey toggles.

Code: `calc.rs` (fend wrapper with time limits), `rates.rs` (exchange rates), `history.rs`, `instance.rs` (toggle socket),
`app.rs` (egui window).
