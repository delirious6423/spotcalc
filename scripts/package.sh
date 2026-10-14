#!/usr/bin/env bash
# Build the release tarball dist/spotcalc-<version>-<arch>-linux.tar.gz with the binary, the
# install script, and every licence text a binary distribution must carry.
# Needs cargo-about: cargo binstall cargo-about (or cargo install --locked cargo-about).
set -euo pipefail
cd "$(dirname "$0")/.."

metadata() { cargo metadata --format-version 1 --locked "$@"; }
VERSION=$(metadata --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
NAME="spotcalc-$VERSION-$(uname -m)-linux"
STAGE="dist/$NAME"

rm -rf "$STAGE" "dist/$NAME.tar.gz"
mkdir -p "$STAGE/licenses/fonts"

cargo build --release --locked
install -m755 target/release/spotcalc "$STAGE/"
install -m755 scripts/install.sh "$STAGE/"
cp LICENSE NOTICE README.md CHANGELOG.md "$STAGE/"

# Licence texts of all compiled-in crates; --fail aborts on any licence not accepted in about.toml.
cargo about generate --locked --fail about.hbs -o "$STAGE/THIRD_PARTY_LICENSES.txt"

# egui's built-in fonts carry their own notices (Ubuntu Font Licence, OFL, Hack's MIT/Bitstream
# Vera, MIT), which cargo-about's crate-level view doesn't fully capture.
fonts_crate=$(metadata | python3 -c '
import json, os, sys
pkgs = json.load(sys.stdin)["packages"]
print(next(os.path.dirname(p["manifest_path"]) for p in pkgs if p["name"] == "epaint_default_fonts"))')
cp "$fonts_crate"/fonts/*.txt "$STAGE/licenses/fonts/"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "Built dist/$NAME.tar.gz"
