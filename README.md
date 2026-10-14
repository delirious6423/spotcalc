# spotcalc

A Spotlight-style calculator popup for GNOME (Wayland or X11). Press a hotkey, type, read the
live result, press Enter to copy it.

Calculation is done by [fend](https://github.com/printfn/fend): arithmetic, units (`3 ft to cm`,
`60 mph to km/h`), percentages (`15% of 80`), bases (`0xff to binary`), dates, variables
(`x = 4`, then `x^2`) and `ans` for the previous result.

## Keys

| Key | Action |
| --- | --- |
| Enter | Calculate, copy the result, save to history |
| ↑ / ↓ | Browse history |
| Esc, hotkey again, or click away | Close |

Clicking a history row copies its result. History: `~/.local/share/spotcalc/history.tsv`.

## Install

```bash
scripts/install.sh '<Super>c'
```

Builds into `~/.cargo/bin/spotcalc` and adds a GNOME custom shortcut (visible in
Settings → Keyboard → Custom Shortcuts). Undo with `scripts/install.sh --uninstall`.

## How it works

Wayland doesn't let apps grab global hotkeys, so the GNOME shortcut simply runs `spotcalc`. The
running instance listens on `$XDG_RUNTIME_DIR/spotcalc.sock`; a second launch connects to it,
which closes the open window, so the hotkey toggles.

Code: `calc.rs` (fend wrapper with time limits), `history.rs`, `instance.rs` (toggle socket),
`app.rs` (egui window).
