# Sourced by run.sh: sets THROWVM to throwvm.py (https://github.com/delirious6423/throwvm), downloaded once at a
# pinned version and checked against a pinned sha256. THROWVM=/path/to/throwvm.py uses a local copy instead.
THROWVM_VERSION=v0.1.0
THROWVM_SHA256=6fcba45eaf5a41b29d4f1082988e00040985fd6f8b4b8e0ad8451e86fe2bedba
if [ -z "${THROWVM:-}" ]; then
    THROWVM="${XDG_CACHE_HOME:-$HOME/.cache}/throwvm/$THROWVM_VERSION/throwvm.py"
    if ! { [ -f "$THROWVM" ] && echo "$THROWVM_SHA256  $THROWVM" | sha256sum -c --status; }; then
        mkdir -p "$(dirname "$THROWVM")"
        new=$(mktemp "$THROWVM.XXXXXX")                 # parallel runs each download their own copy
        curl -fsSL --retry 3 --max-time 60 -o "$new" \
            "${THROWVM_URL:-https://raw.githubusercontent.com/delirious6423/throwvm/$THROWVM_VERSION/throwvm.py}"
        echo "$THROWVM_SHA256  $new" | sha256sum -c --status \
            || { echo "throwvm.py $THROWVM_VERSION: checksum mismatch" >&2; rm -f "$new"; exit 1; }
        mv "$new" "$THROWVM"
    fi
fi
