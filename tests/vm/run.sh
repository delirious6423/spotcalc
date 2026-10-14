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
# Needs qemu-system-x86 and qemu-utils, access to /dev/kvm, and cargo-about + cargo-deb.
# VM_DIR holds the cached Ubuntu image and a prepared image with the test tools installed;
# set it to a disk with room (the Ubuntu image is shared with other VM tests in VM_DIR/images).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
: "${VM_DIR:?Set VM_DIR to a folder with ~5 GB free for VM images}"
RELEASE=resolute VERSION=26.04
IMG="ubuntu-$VERSION-server-cloudimg-amd64.img"
URL="https://cloud-images.ubuntu.com/releases/$RELEASE/release"
SSH_PORT=2223 SEED_PORT=8766
# Test tools inside the VM. Changing this list rebuilds the prepared image.
PACKAGES="sway grim wtype wl-clipboard wlrctl jq tesseract-ocr imagemagick fonts-dejavu-core
libgl1-mesa-dri libegl-mesa0 libglib2.0-bin gsettings-desktop-schemas gnome-settings-daemon-common
desktop-file-utils dbus-user-session"
KEEP=0 BUILD=1
for a in "$@"; do
    case "$a" in --keep) KEEP=1 ;; --no-build) BUILD=0 ;; *) echo "unknown: $a" >&2; exit 2 ;; esac
done

step() { printf '\n\033[1m### %s\033[0m\n' "$*"; }
[ -r /dev/kvm ] && [ -w /dev/kvm ] || { echo "no access to /dev/kvm" >&2; exit 1; }
command -v qemu-system-x86_64 qemu-img >/dev/null || { echo "Needs: sudo apt install qemu-system-x86 qemu-utils" >&2; exit 1; }

if [ "$BUILD" = 1 ]; then
    step "Build packages"
    "$ROOT/scripts/package.sh" > /dev/null
fi
DEB=$(ls "$ROOT"/dist/spotcalc_*.deb)
OUT="$ROOT/target/vm-test"
rm -rf "$OUT" && mkdir -p "$OUT"

step "Ubuntu $VERSION cloud image (signature and checksum checked)"
mkdir -p "$VM_DIR/images" && cd "$VM_DIR/images"
curl -fsS --max-time 60 -O "$URL/SHA256SUMS" -O "$URL/SHA256SUMS.gpg"
gpgv --keyring /usr/share/keyrings/ubuntu-cloudimage-keyring.gpg SHA256SUMS.gpg SHA256SUMS 2>/dev/null
if ! grep " \*$IMG\$" SHA256SUMS | sed 's/\*//' | sha256sum -c --status 2>/dev/null; then
    curl -fL --retry 3 --max-time 1800 -o "$IMG" "$URL/$IMG"
    grep " \*$IMG\$" SHA256SUMS | sed 's/\*//' | sha256sum -c
fi

# One VM instance: a disk overlay plus cloud-init seed served over HTTP (no ISO tools needed).
W="$VM_DIR/spotcalc"
mkdir -p "$W"
[ -f "$W/id_test" ] || ssh-keygen -q -t ed25519 -N '' -C spotcalc-vm-test -f "$W/id_test"
SSH=(ssh -q -i "$W/id_test" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -p "$SSH_PORT" tester@127.0.0.1)
SEED_PID="" VM_PID=""
boot() { # boot <disk> <seed dir>
    python3 -I -m http.server "$SEED_PORT" --bind 127.0.0.1 --directory "$2" >/dev/null 2>&1 & SEED_PID=$!
    qemu-system-x86_64 -enable-kvm -cpu host -smp 4 -m 4096 -nographic -monitor none \
        -serial "file:$W/console.log" -drive "file=$1,if=virtio" \
        -nic "user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
        -smbios "type=1,serial=ds=nocloud;s=http://10.0.2.2:$SEED_PORT/" >/dev/null 2>&1 & VM_PID=$!
    timeout 300 bash -c "until ${SSH[*]} -o ConnectTimeout=3 true 2>/dev/null; do sleep 3; done"
    "${SSH[@]}" 'cloud-init status --wait >/dev/null || true'
}
halt() {
    [ -n "$SEED_PID" ] && kill "$SEED_PID" 2>/dev/null || true
    [ -n "$VM_PID" ] && kill "$VM_PID" 2>/dev/null || true
    [ -n "$VM_PID" ] && while kill -0 "$VM_PID" 2>/dev/null; do sleep 0.5; done
    SEED_PID="" VM_PID=""
}
trap '[ "$KEEP" = 1 ] || { halt; rm -f "$W/run.qcow2"; }' EXIT

# Same seed (and instance id) for every boot, so the prepared image never re-runs first-boot setup.
mkdir -p "$W/seed"
cat > "$W/seed/user-data" <<EOF
#cloud-config
hostname: spotcalc-test
users:
  - name: tester
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys: ["$(cat "$W/id_test.pub")"]
ssh_pwauth: false
EOF
printf 'instance-id: spotcalc-test\nlocal-hostname: spotcalc-test\n' > "$W/seed/meta-data"
: > "$W/seed/vendor-data"

TOOLS_HASH=$(printf '%s %s' "$IMG" "$PACKAGES" | sha256sum | cut -c1-12)
BASE="$W/base-$TOOLS_HASH.qcow2"
if [ ! -f "$BASE" ]; then
    step "Prepare VM image with test tools (once; cached as $(basename "$BASE"))"
    rm -f "$W"/base-*.qcow2
    qemu-img create -q -f qcow2 -F qcow2 -b "$VM_DIR/images/$IMG" "$BASE.tmp" 20G
    boot "$BASE.tmp" "$W/seed"
    # shellcheck disable=SC2086 # word-splitting the package list is intended
    "${SSH[@]}" "sudo DEBIAN_FRONTEND=noninteractive apt-get update -q >/dev/null &&
        sudo DEBIAN_FRONTEND=noninteractive apt-get install -yq --no-install-recommends $(echo $PACKAGES) >/dev/null &&
        sudo loginctl enable-linger tester" > "$W/prepare.log" 2>&1 \
        || { tail -20 "$W/prepare.log"; echo "FAIL  preparing image"; exit 1; }
    "${SSH[@]}" 'sudo sync; sudo poweroff' || true
    while kill -0 "$VM_PID" 2>/dev/null; do sleep 1; done
    halt
    mv "$BASE.tmp" "$BASE"
fi

step "Fresh VM"
rm -f "$W/run.qcow2"
qemu-img create -q -f qcow2 -F qcow2 -b "$BASE" "$W/run.qcow2"
boot "$W/run.qcow2" "$W/seed"
"${SSH[@]}" '. /etc/os-release; echo "$PRETTY_NAME, $(nproc) CPUs, no GPU (Mesa software rendering)"'

step "Checks inside the VM"
"${SSH[@]}" 'cat > /tmp/spotcalc.deb' < "$DEB"
RESULT=0
"${SSH[@]}" 'bash -s' < "$ROOT/tests/vm/guest.sh" | tee "$OUT/checks.log" || RESULT=$?
"${SSH[@]}" 'cd /tmp/vm-test && tar -cf - .' | tar -xf - -C "$OUT" || true
echo
echo "Logs and screenshots: $OUT"
[ "$KEEP" = 1 ] && echo "VM kept running: ${SSH[*]}   (stop: kill $VM_PID)"
exit $RESULT
