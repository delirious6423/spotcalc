#!/usr/bin/env bash
# End-to-end test of the real .deb in a throwaway Ubuntu 26.04 VM (QEMU/KVM), driving the actual
# window in a headless Sway session: typing, clicks, clipboard, history, toggle, focus handling,
# idle redraws, memory and the hotkey banner. Your own desktop is never touched.
#
#   VM_DIR=/path/with/5GB tests/vm/run.sh          build packages, run all checks, delete the VM
#   VM_DIR=... tests/vm/run.sh --keep              leave the VM running afterwards (SSH printed)
#   VM_DIR=... tests/vm/run.sh --no-build          reuse the packages already in dist/
#
# Writes test logs and screenshots to target/vm-test/, including the README screenshot.
# Needs qemu-system-x86 and qemu-utils, access to /dev/kvm, a systemd user session, and cargo-about + cargo-deb.
# The VM comes from throwvm (tests/vm/throwvm-pin.sh fetches a pinned version): VM_DIR holds the cached Ubuntu
# image and snapshots, one after first boot and one with the test tools of tests/vm/provision.sh. The first run
# makes them (a few minutes); later runs resume the VM in about a second. On failure the VM's console log and
# journal stay in VM_DIR/spotcalc-test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${VM_DIR:?Set VM_DIR to a folder with ~5 GB free for VM images}"
export VM_DIR
KEEP=0 BUILD=1
for a in "$@"; do
    case "$a" in --keep) KEEP=1 ;; --no-build) BUILD=0 ;; *) echo "unknown: $a" >&2; exit 2 ;; esac
done

step() { printf '\n\033[1m### %s\033[0m\n' "$*"; }
[ -r /dev/kvm ] && [ -w /dev/kvm ] || { echo "no access to /dev/kvm" >&2; exit 1; }
command -v qemu-system-x86_64 qemu-img >/dev/null || { echo "Needs: sudo apt install qemu-system-x86 qemu-utils" >&2; exit 1; }
# shellcheck source=throwvm-pin.sh
. "$ROOT/tests/vm/throwvm-pin.sh"

if [ "$BUILD" = 1 ]; then
    step "Build packages"
    "$ROOT/scripts/package.sh" > /dev/null
fi
DEB=$(ls "$ROOT"/dist/spotcalc_*.deb)
OUT="$ROOT/target/vm-test"
rm -rf "$OUT" && mkdir -p "$OUT"

step "Fresh VM"
W="$VM_DIR/spotcalc-test"
rm -rf "$W" && mkdir -p "$W"
cleanup() {     # unless --keep: stop the VM; on failure keep its logs in $W
    local rc=$?
    [ -f "$W/vm.json" ] || return 0
    if [ "$rc" != 0 ]; then python3 -I "$THROWVM" down "$W" --failed
    elif [ "$KEEP" = 0 ]; then python3 -I "$THROWVM" down "$W" --rm; fi
}
trap cleanup EXIT
OWNER=$$; [ "$KEEP" = 1 ] && OWNER=0
VM_ENV=$(python3 -I "$THROWVM" up "$W" --name spotcalc --hostname spotcalc-test --mem 4096 --cpus 4 \
    --provision "$ROOT/tests/vm/provision.sh" --owner "$OWNER")   # an assignment, so set -e stops here on failure
eval "$VM_ENV"
SSH=("${VM_SSH[@]}")
"${SSH[@]}" '. /etc/os-release; echo "$PRETTY_NAME, $(nproc) CPUs, no GPU (Mesa software rendering)"'

step "Checks inside the VM"
"${SSH[@]}" 'cat > /tmp/spotcalc.deb' < "$DEB"
RESULT=0
"${SSH[@]}" 'bash -s' < "$ROOT/tests/vm/guest.sh" | tee "$OUT/checks.log" || RESULT=$?
"${SSH[@]}" 'cd /tmp/vm-test && tar -cf - .' | tar -xf - -C "$OUT" || true
echo
echo "Logs and screenshots: $OUT"
[ "$KEEP" = 1 ] && echo "VM kept running: ${SSH[*]}   (stop: python3 -I $THROWVM down $W --rm)"
exit $RESULT
