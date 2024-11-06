#!/usr/bin/env bash
set -ex
SCRIPT_DIR="$(readlink -f -- "${BASH_SOURCE%/*}")"
WIFI=$(ipconfig getsummary en0 | awk '/ SSID /{print $3}')
test "" != "$WIFI"
for elt in ssid psk; do
    ln -sf ".$WIFI.$elt" "$SCRIPT_DIR/$elt"
done
