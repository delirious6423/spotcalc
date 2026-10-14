#!/usr/bin/env bash
# Runs inside the test VM (started by run.sh): installs the .deb, starts a headless Sway session
# and drives the real spotcalc window with simulated keys and clicks. Reads the window back with
# screenshots + OCR and the clipboard. Results and screenshots go to /tmp/vm-test.
set -uo pipefail

OUT=/tmp/vm-test
rm -rf "$OUT" && mkdir -p "$OUT"
FAIL=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAIL=1; }
info() { echo "INFO  $*"; }
check() { # check <name> <command...>
    local name=$1; shift
    if "$@" > "$OUT/last.out" 2>&1; then pass "$name"; else fail "$name"; sed 's/^/      /' "$OUT/last.out" | tail -8; fi
}

# --- Install ---------------------------------------------------------------------------------
check "apt installs the .deb with its dependencies" \
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq /tmp/spotcalc.deb
check "desktop entry is valid" desktop-file-validate /usr/share/applications/spotcalc.desktop
check "icon installed" test -s /usr/share/icons/hicolor/scalable/apps/spotcalc.svg
check "licence texts installed" test -s /usr/share/doc/spotcalc/THIRD_PARTY_LICENSES.txt

# --- Headless Sway session ---------------------------------------------------------------------
export XDG_RUNTIME_DIR="/run/user/$(id -u)"
mkdir -p ~/.config/sway
cat > ~/.config/sway/config <<'EOF'
output * mode 2560x1600 scale 2
default_floating_border none
for_window [app_id="spotcalc"] floating enable
EOF
WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway > "$OUT/sway.log" 2>&1 &
for _ in $(seq 100); do
    SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.*.sock 2>/dev/null | head -1)
    [ -n "$SWAYSOCK" ] && break
    sleep 0.1
done
export SWAYSOCK
WAYLAND_DISPLAY=$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-? | head -1)")
export WAYLAND_DISPLAY
check "headless Sway session running" swaymsg -t get_version

# Keep one virtual keyboard alive for the whole session. Each wtype call creates and destroys its own
# virtual keyboard; with no other keyboard Sway drops the seat's keyboard capability when wtype exits,
# the window loses focus and closes itself. F24 is held (never released) and ignored by the app.
wtype -P F24 -s 3600000 -p F24 > /dev/null 2>&1 &
HOLD_PID=$!
# Same for the pointer: wlrctl's virtual pointer vanishes when it exits, and a click sent before the app
# has bound the new wl_pointer is lost. This holds one virtual pointer open for the whole session.
cat > "$OUT/hold-pointer.py" <<'PY'
# Holds one virtual pointer (wlr-virtual-pointer-unstable-v1) alive on the Wayland display, using the
# raw wire protocol so no extra packages are needed. See guest.sh for why.
import os, socket, struct, sys, time

s = socket.socket(socket.AF_UNIX)
s.connect(os.path.join(os.environ["XDG_RUNTIME_DIR"], os.environ["WAYLAND_DISPLAY"]))

def send(obj, op, payload=b""):
    s.sendall(struct.pack("II", obj, ((8 + len(payload)) << 16) | op) + payload)

def string(v):
    b = v.encode() + b"\0"
    return struct.pack("I", len(b)) + b + b"\0" * (-len(b) % 4)

buf = b""
def events():
    global buf
    buf += s.recv(65536)
    while len(buf) >= 8:
        obj, sz_op = struct.unpack("II", buf[:8])
        size, op = sz_op >> 16, sz_op & 0xFFFF
        if len(buf) < size:
            return
        yield obj, op, buf[8:size]
        buf = buf[size:]

send(1, 1, struct.pack("I", 2))  # wl_display.get_registry -> id 2
send(1, 0, struct.pack("I", 3))  # wl_display.sync -> callback id 3
globals_ = {}
done = False
while not done:
    for obj, op, data in events():
        if obj == 2 and op == 0:  # wl_registry.global
            name, n = struct.unpack("II", data[:8])
            iface = data[8:8 + n - 1].decode()
            globals_[iface] = name
        elif obj == 3:
            done = True

def bind(iface, new_id):
    send(2, 0, struct.pack("I", globals_[iface]) + string(iface) + struct.pack("II", 1, new_id))

bind("wl_seat", 4)
bind("zwlr_virtual_pointer_manager_v1", 5)
send(5, 0, struct.pack("II", 4, 6))  # create_virtual_pointer(seat, id 6)
send(1, 0, struct.pack("I", 7))
print("ready", flush=True)
while True:
    for _ in events():
        pass
PY
python3 "$OUT/hold-pointer.py" > "$OUT/hold-pointer.log" 2>&1 &
HOLD_PTR_PID=$!
for _ in $(seq 50); do grep -q ready "$OUT/hold-pointer.log" && break; sleep 0.1; done
sleep 0.5

# Main tests run with the hotkey banner already dismissed; the banner has its own tests below.
mkdir -p ~/.config/spotcalc && touch ~/.config/spotcalc/hotkey-prompt-dismissed

win() { swaymsg -t get_tree | jq -c 'first(.. | objects | select(.app_id? == "spotcalc"))'; }
wait_window() { for _ in $(seq 200); do [ -n "$(win)" ] && return 0; sleep 0.025; done; return 1; }
# eventually <command...>: retry for up to 5 s (the window redraws a moment after input).
eventually() { for _ in $(seq 25); do "$@" && return 0; sleep 0.2; done; return 1; }
# The app only closes on focus loss once it has seen focus, so wait until Sway focuses it and the app has
# had a moment to process the event.
wait_focused() { eventually bash -c 'win | jq -e ".focused" > /dev/null' && sleep 0.5; }
wait_gone() { for _ in $(seq 80); do pgrep -x spotcalc > /dev/null || return 0; sleep 0.05; done; return 1; }
start() { RUST_LOG=spotcalc=trace "${@:2}" spotcalc > "$OUT/$1.log" 2>&1 & }
# Screenshot of just the window, at the output's 2x scale.
shot() { grim -g "$(win | jq -r '.rect | "\(.x),\(.y) \(.width)x\(.height)"')" "$OUT/$1.png"; }
# Text in the window: light-on-dark, so invert for tesseract.
ocr() { shot "$1" && magick "$OUT/$1.png" -negate "$OUT/$1-ocr.png" && tesseract "$OUT/$1-ocr.png" - 2>/dev/null; }
export OUT
export -f win shot ocr
type_keys() { wtype "$1"; sleep 0.4; }
key() { wtype -k "$1"; sleep 0.3; }
# Clicks the centre of the first OCR word matching regex $2, in screenshot $1 (pixels are 2x logical).
click_word() {
    local box
    box=$(tesseract "$OUT/$1-ocr.png" - tsv 2>/dev/null | awk -F'\t' -v w="$2" '$12 ~ w {print $7, $8, $9, $10; exit}')
    [ -n "$box" ] || { echo "word '$2' not found"; return 1; }
    read -r left top width height <<< "$box"
    local x y
    x=$(win | jq ".rect.x + ($left + $width / 2) / 2 | floor")
    y=$(win | jq ".rect.y + ($top + $height / 2) / 2 | floor")
    wlrctl pointer move -10000 -10000 && wlrctl pointer move "$x" "$y" && wlrctl pointer click left
    sleep 0.5
}

# --- Startup -----------------------------------------------------------------------------------
t0=$(date +%s%N)
start main
check "window opens" wait_window
info "startup to visible window: $(( ($(date +%s%N) - t0) / 1000000 )) ms (software rendering in a VM)"
check "window floats, has focus, 640x380" \
    bash -c "$(declare -f win); win | jq -e '.focused and .type == \"floating_con\" and .rect.width == 640 and .rect.height == 380'"

# --- Calculations ------------------------------------------------------------------------------
type_keys "15% of 80"
check "live result shows while typing" eventually bash -c "ocr preview | grep -qE '= ?12'"
key Return
clip_is() { [ "$(wl-paste -n 2>/dev/null)" = "$1" ]; }
check "Enter copies the result" eventually clip_is "12"
check "history saved" eventually grep -qxP '15% of 80\t12' ~/.local/share/spotcalc/history.tsv

type_keys "3 ft to cm"; key Return
check "units convert" eventually clip_is "91.44 cm"
type_keys "0xff to binary"; key Return
check "number bases convert" eventually clip_is "11111111"

for _ in $(seq 60); do [ -s ~/.cache/spotcalc/rates.json ] && break; sleep 0.25; done
check "exchange rates downloaded in the background" test -s ~/.cache/spotcalc/rates.json
type_keys "100 USD to TWD"; key Return
check "currency converts" eventually bash -c "wl-paste -n | grep -xE '[0-9]+(\.[0-9]+)? TWD'"

key Up
check "Up recalls the last calculation" eventually bash -c "ocr recall | head -3 | grep -qF '100 USD to TWD'"
key Down

# --- Efficiency --------------------------------------------------------------------------------
PID=$(pgrep -x spotcalc | head -1)
sleep 1
frames0=$(grep -c 'frame' "$OUT/main.log")
cpu0=$(awk '{print $14 + $15}' "/proc/$PID/stat")
sleep 5
frames=$(( $(grep -c 'frame' "$OUT/main.log") - frames0 ))
cpu=$(( $(awk '{print $14 + $15}' "/proc/$PID/stat") - cpu0 ))
check "no redraws while idle (5 s: $frames frames)" test "$frames" -eq 0
check "no CPU while idle (5 s: $cpu ticks)" test "$cpu" -le 1
info "memory: $(( $(awk '/VmRSS/{print $2}' "/proc/$PID/status") / 1024 )) MB RSS (software rendering)"

# --- Closing -----------------------------------------------------------------------------------
spotcalc
check "running spotcalc again closes the window (hotkey toggle)" wait_gone
check "toggle socket cleaned up" test ! -e "$XDG_RUNTIME_DIR/spotcalc.sock"

start esc; wait_window; key Escape
check "Esc closes" wait_gone

# Focus moves to a second (empty) output, so the window stays visible. Moving to another workspace instead
# would hide it, and Sway then sends no frame callbacks, so the app doesn't get to react until shown again.
swaymsg -q create_output
start focus; wait_window; wait_focused; swaymsg -q focus output HEADLESS-2
check "losing focus closes" wait_gone
swaymsg -q focus output HEADLESS-1

# --- Hotkey banner (GNOME settings in a throwaway keyfile) ----------------------------------------
cfg=$(mktemp -d)
start banner env GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$cfg"
wait_window; sleep 1.5
ocr banner > "$OUT/banner.txt"
check "first launch offers Alt+Space" grep -F "Open spotcalc anytime with Alt+Space" "$OUT/banner.txt"
check "clicking Set up works" click_word banner '^Set'
keyfile="$cfg/glib-2.0/settings/keyfile"
check "shortcut written: Alt+Space runs /usr/bin/spotcalc" \
    bash -c "grep -qx \"binding='<Alt>space'\" $keyfile && grep -qx \"command='/usr/bin/spotcalc'\" $keyfile"
check "window menu freed from Alt+Space" grep -qx 'activate-window-menu=\[\]' "$keyfile"
check "banner confirms" eventually bash -c "ocr banner-done | grep -qF 'Done: press Alt+Space'"
key Escape; wait_gone

cfg=$(mktemp -d)
start notnow env GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$cfg"
wait_window; sleep 1.5; ocr notnow > /dev/null
check "clicking Not now works" click_word notnow '^Not'
check "Not now is remembered" test -e "$cfg/spotcalc/hotkey-prompt-dismissed"
key Escape; wait_gone
start notnow2 env GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$cfg"
wait_window; sleep 1.5
check "no banner after Not now" bash -c "! ocr notnow2 | grep -qE 'Set ?up'"
key Escape; wait_gone

# --- README screenshot -------------------------------------------------------------------------
start screenshot; wait_window
type_keys "70 kg to lb"
sleep 0.5; shot readme-screenshot
check "README screenshot taken" test -s "$OUT/readme-screenshot.png"
key Escape; wait_gone

# --- Uninstall ---------------------------------------------------------------------------------
check "apt removes it" sudo DEBIAN_FRONTEND=noninteractive apt-get remove -yq spotcalc
check "binary and desktop entry gone" \
    bash -c "! test -e /usr/bin/spotcalc && ! test -e /usr/share/applications/spotcalc.desktop"

kill "$HOLD_PID" "$HOLD_PTR_PID" 2>/dev/null
echo
[ "$FAIL" = 0 ] && echo "All checks passed." || echo "Some checks FAILED."
exit "$FAIL"
