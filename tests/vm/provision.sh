#!/usr/bin/env bash
# Test tools for the VM test, installed once on top of the shared Ubuntu snapshot (run as root, with network).
# throwvm saves the result as a snapshot layer; changing this file rebuilds that layer on the next run.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -q >/dev/null
apt-get install -yq --no-install-recommends \
    sway grim wtype wl-clipboard wlrctl jq tesseract-ocr imagemagick fonts-dejavu-core \
    libgl1-mesa-dri libegl-mesa0 libglib2.0-bin gsettings-desktop-schemas gnome-settings-daemon-common \
    desktop-file-utils dbus-user-session >/dev/null
loginctl enable-linger tester     # tester's user session (XDG_RUNTIME_DIR, D-Bus) runs without a login
