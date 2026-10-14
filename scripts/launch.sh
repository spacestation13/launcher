#!/bin/bash
# Launch script for SS13 Launcher (sharun portable format)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

export APPDIR="$SCRIPT_DIR"

export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"

# Create path compatibility symlink for x86_64-linux-gnu
if [ ! -e "$SCRIPT_DIR/lib/x86_64-linux-gnu" ]; then
    ln -sf . "$SCRIPT_DIR/lib/x86_64-linux-gnu"
fi

if [ -f "$SCRIPT_DIR/bin/01-path-mapping-hardcoded.hook" ]; then
    for _dir in bin lib share; do
        _name=$(grep "^_tmp_${_dir}=" "$SCRIPT_DIR/bin/01-path-mapping-hardcoded.hook" | cut -d'=' -f2- | tr -d '"')
        if [ -n "$_name" ]; then
            ln -sfn "$SCRIPT_DIR/$_dir" "/tmp/$_name" 2>/dev/null || true
            if [ ! -L "/tmp/$_name" ] || [ ! -O "/tmp/$_name" ] \
                || [ "$(readlink "/tmp/$_name")" != "$SCRIPT_DIR/$_dir" ]; then
                echo "Error: /tmp/$_name is not a symlink to $SCRIPT_DIR/$_dir owned by you, refusing to launch." >&2
                echo "Remove it (or ask its owner to) and try again." >&2
                exit 1
            fi
        fi
    done

    _tmp_lib=$(grep '_tmp_lib=' "$SCRIPT_DIR/bin/01-path-mapping-hardcoded.hook" | cut -d'=' -f2 | tr -d '"')
    if [ -n "$_tmp_lib" ]; then
        mkdir -p "$SCRIPT_DIR/tmp"
        if [ ! -e "$SCRIPT_DIR/tmp/$_tmp_lib" ]; then
            ln -sf "$SCRIPT_DIR/lib" "$SCRIPT_DIR/tmp/$_tmp_lib"
        fi
    fi
fi

exec "$SCRIPT_DIR/AppRun" "$@"
