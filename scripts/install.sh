#!/usr/bin/env bash
# Install spotcalc to ~/.cargo/bin and bind a GNOME keyboard shortcut to it.
# Usage: scripts/install.sh ['<Alt>space']     Undo: scripts/install.sh --uninstall
set -euo pipefail

KEY="${1:-<Alt>space}"
SCHEMA=org.gnome.settings-daemon.plugins.media-keys
BASE=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
ENTRY="$BASE/spotcalc/"
CUSTOM="$SCHEMA.custom-keybinding:$ENTRY"

current=$(gsettings get "$SCHEMA" custom-keybindings)
# GNOME binds <Alt>space to the window menu; a custom shortcut on the same key would never fire.
WM=org.gnome.desktop.wm.keybindings

if [[ "$KEY" == "--uninstall" ]]; then
    new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print([x for x in l if x!=sys.argv[2]])" "$current" "$ENTRY")
    gsettings set "$SCHEMA" custom-keybindings "$new"
    gsettings reset-recursively "$CUSTOM"
    gsettings reset "$WM" activate-window-menu
    cargo uninstall spotcalc || true
    echo "Removed the shortcut and the binary. History remains in ~/.local/share/spotcalc/."
    exit 0
fi

cargo install --locked --path "$(dirname "$0")/.."

if [[ "$(gsettings get "$WM" activate-window-menu)" == *"'$KEY'"* ]]; then
    gsettings set "$WM" activate-window-menu "[]"
    echo "Freed $KEY from the window menu (right-click a title bar to open it)."
fi

if [[ "$current" != *"$ENTRY"* ]]; then
    new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print(l+[sys.argv[2]])" "$current" "$ENTRY")
    gsettings set "$SCHEMA" custom-keybindings "$new"
fi
# On NVIDIA-only machines, skip loading Mesa's EGL driver too (it drags in ~45 MB of LLVM):
# measured 120 -> 69 MB RSS and steadier startup. Re-run this script if the GPU setup changes.
COMMAND="$HOME/.cargo/bin/spotcalc"
NVIDIA_EGL=/usr/share/glvnd/egl_vendor.d/10_nvidia.json
gpu_vendors=$(cat /sys/class/drm/card*/device/vendor 2>/dev/null | sort -u)
if [[ -f "$NVIDIA_EGL" && "$gpu_vendors" == "0x10de" ]]; then
    COMMAND="env __EGL_VENDOR_LIBRARY_FILENAMES=$NVIDIA_EGL $COMMAND"
    echo "NVIDIA-only GPU: the shortcut loads only NVIDIA's EGL driver."
fi

gsettings set "$CUSTOM" name 'spotcalc'
gsettings set "$CUSTOM" command "$COMMAND"
gsettings set "$CUSTOM" binding "$KEY"
echo "Press $KEY to open spotcalc (press it again or Esc to close)."
