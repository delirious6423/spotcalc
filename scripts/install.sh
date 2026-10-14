#!/usr/bin/env bash
# Install spotcalc and bind a GNOME keyboard shortcut to it.
#
# From a release download: copies the bundled binary to ~/.local/bin (no Rust needed).
# From a source checkout: builds with cargo into ~/.cargo/bin.
#
# Usage: ./install.sh ['<Alt>space']     Undo: ./install.sh --uninstall
set -euo pipefail

KEY="${1:-<Alt>space}"
HERE="$(cd "$(dirname "$0")" && pwd)"
SCHEMA=org.gnome.settings-daemon.plugins.media-keys
BASE=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings
ENTRY="$BASE/spotcalc/"
CUSTOM="$SCHEMA.custom-keybinding:$ENTRY"
# GNOME binds <Alt>space to the window menu; a custom shortcut on the same key would never fire.
WM=org.gnome.desktop.wm.keybindings
LOCAL_BIN="$HOME/.local/bin/spotcalc"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_FILE="$DATA/applications/spotcalc.desktop"
ICON_FILE="$DATA/icons/hicolor/scalable/apps/spotcalc.svg"

have_gnome() { gsettings list-schemas 2>/dev/null | grep -qx "$SCHEMA"; }

if [[ "$KEY" == "--uninstall" ]]; then
    if have_gnome; then
        current=$(gsettings get "$SCHEMA" custom-keybindings)
        new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print([x for x in l if x!=sys.argv[2]])" "$current" "$ENTRY")
        gsettings set "$SCHEMA" custom-keybindings "$new"
        gsettings reset-recursively "$CUSTOM"
        gsettings reset "$WM" activate-window-menu
    fi
    rm -f "$LOCAL_BIN" "$DESKTOP_FILE" "$ICON_FILE"
    if command -v cargo >/dev/null; then cargo uninstall spotcalc 2>/dev/null || true; fi
    echo "Removed spotcalc and its shortcut."
    echo "History and cached rates remain in ~/.local/share/spotcalc and ~/.cache/spotcalc."
    exit 0
fi

if [[ -x "$HERE/spotcalc" ]]; then
    install -Dm755 "$HERE/spotcalc" "$LOCAL_BIN"
    BINARY="$LOCAL_BIN"
    SHARE="$HERE/share"
else
    cargo install --locked --path "$HERE/.."
    BINARY="$HOME/.cargo/bin/spotcalc"
    SHARE="$HERE/../assets"
fi
echo "Installed $BINARY"

# App-grid entry and icon. Exec gets the full path, since ~/.local/bin and ~/.cargo/bin
# aren't always on the desktop session's PATH.
install -Dm644 "$SHARE/spotcalc.svg" "$ICON_FILE"
mkdir -p "$(dirname "$DESKTOP_FILE")"
sed "s|^Exec=spotcalc$|Exec=$BINARY|" "$SHARE/spotcalc.desktop" > "$DESKTOP_FILE"

if ! have_gnome; then
    echo "Not GNOME: bind a key in your desktop's settings to run $BINARY (see README.md)."
    exit 0
fi

if [[ "$(gsettings get "$WM" activate-window-menu)" == *"'$KEY'"* ]]; then
    gsettings set "$WM" activate-window-menu "[]"
    echo "Freed $KEY from the window menu (right-click a title bar to open it)."
fi

current=$(gsettings get "$SCHEMA" custom-keybindings)
if [[ "$current" != *"$ENTRY"* ]]; then
    new=$(python3 -c "import ast,sys; l=ast.literal_eval(sys.argv[1].replace('@as ','')); print(l+[sys.argv[2]])" "$current" "$ENTRY")
    gsettings set "$SCHEMA" custom-keybindings "$new"
fi

# On NVIDIA-only machines, skip loading Mesa's EGL driver too (it drags in ~45 MB of LLVM):
# measured 120 -> 69 MB RSS and steadier startup. Re-run this script if the GPU setup changes.
COMMAND="$BINARY"
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
