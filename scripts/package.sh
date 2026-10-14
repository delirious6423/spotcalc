#!/usr/bin/env bash
# Build the release packages in dist/:
#   spotcalc-<version>-<arch>-linux.tar.gz   binary + install.sh, for any distro
#   spotcalc_<version>-1_<debarch>.deb        double-click install on Debian/Ubuntu
# Both carry every licence text a binary distribution must include.
# Needs cargo-about and cargo-deb: cargo binstall cargo-about cargo-deb
set -euo pipefail
cd "$(dirname "$0")/.."

metadata() { cargo metadata --format-version 1 --locked "$@"; }
VERSION=$(metadata --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
NAME="spotcalc-$VERSION-$(uname -m)-linux"
STAGE="dist/$NAME"
LICENSES=dist/licenses

rm -rf dist
mkdir -p "$STAGE" "$LICENSES/fonts"

# Licence texts of all compiled-in crates; --fail aborts on any licence not accepted in about.toml.
cargo about generate --locked --fail about.hbs -o "$LICENSES/THIRD_PARTY_LICENSES.txt"

# egui's built-in fonts carry their own notices (Ubuntu Font Licence, OFL, Hack's MIT/Bitstream
# Vera, MIT), which cargo-about's crate-level view doesn't fully capture.
fonts_crate=$(metadata | python3 -c '
import json, os, sys
pkgs = json.load(sys.stdin)["packages"]
print(next(os.path.dirname(p["manifest_path"]) for p in pkgs if p["name"] == "epaint_default_fonts"))')
cp "$fonts_crate"/fonts/*.txt "$LICENSES/fonts/"

cargo build --release --locked

# Tarball
install -m755 target/release/spotcalc scripts/install.sh "$STAGE/"
cp LICENSE NOTICE README.md CHANGELOG.md "$STAGE/"
mkdir -p "$STAGE/share"
cp assets/spotcalc.desktop assets/spotcalc.svg "$STAGE/share/"
cp -r "$LICENSES" "$STAGE/licenses"
mv "$STAGE/licenses/THIRD_PARTY_LICENSES.txt" "$STAGE/"
tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"

# Debian package (assets listed under [package.metadata.deb] in Cargo.toml)
cargo deb --locked --no-build --output dist/

cd dist
sha256sum -- *.tar.gz *.deb > SHA256SUMS
echo "Built:"
ls -1 -- *.tar.gz *.deb SHA256SUMS
