#!/usr/bin/env bash
# Install spotcalc to ~/.cargo/bin and bind a GNOME keyboard shortcut to it.
# Usage: scripts/install.sh ['<Super>c']     Undo: scripts/install.sh --uninstall
set -euo pipefail

KEY="${1:-<Super>c}"
SCHEMA=org.gnome.settings-daemon.plugins.media-keys
BASE=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
ENTRY="$BASE/spotcalc/"
CUSTOM="$SCHEMA.custom-keybinding:$ENTRY"

current=$(gsettings get "$SCHEMA" custom-keybindings)

if [[ "$KEY" == "--uninstall" ]]; then
    new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print([x for x in l if x!=sys.argv[2]])" "$current" "$ENTRY")
    gsettings set "$SCHEMA" custom-keybindings "$new"
    gsettings reset-recursively "$CUSTOM"
    cargo uninstall spotcalc || true
    echo "Removed the shortcut and the binary. History remains in ~/.local/share/spotcalc/."
    exit 0
fi

cargo install --locked --path "$(dirname "$0")/.."

if [[ "$current" != *"$ENTRY"* ]]; then
    new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print(l+[sys.argv[2]])" "$current" "$ENTRY")
    gsettings set "$SCHEMA" custom-keybindings "$new"
fi
gsettings set "$CUSTOM" name 'spotcalc'
gsettings set "$CUSTOM" command "$HOME/.cargo/bin/spotcalc"
gsettings set "$CUSTOM" binding "$KEY"
echo "Press $KEY to open spotcalc (press it again or Esc to close)."
