#!/bin/bash

set -euo pipefail

manifest="${1:-Cargo.toml}"

if [ ! -f "$manifest" ]; then
    echo "Manifest not found: $manifest"
    exit 1
fi

tmp="$(mktemp)"

awk '
    BEGIN {
        in_dev_dependencies = 0
    }
    /^\[dev-dependencies\]/ {
        in_dev_dependencies = 1
        print
        next
    }
    /^\[/ {
        in_dev_dependencies = 0
    }
    {
        if (in_dev_dependencies && $0 ~ /^magicblock-delegation-program = \{/) {
            gsub(/version = "[^"]+", /, "")
        }
        print
    }
' "$manifest" > "$tmp"

if cmp -s "$manifest" "$tmp"; then
    rm -f "$tmp"
    echo "No publish manifest changes needed for $manifest"
    exit 0
fi

mv "$tmp" "$manifest"
echo "Prepared $manifest for publish"
